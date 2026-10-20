//! Host functions: Rust closures as JavaScript functions. The core runs the
//! body this module wraps around the embedder's closure; the closure reads
//! its call and calls back into the engine through a [`Scope`].

use std::rc::{Rc, Weak};

use oxide_core::vm::native::{HostFn, NativeCall};
use oxide_core::{Intrinsic, NativeCx};

use super::RuntimeError;
use super::ops::{JsResult, View};
use super::runtime::Shared;
use super::value::{CallableRef, Value};

/// A host function's view of the engine: its call, and every operation of
/// [`Js`](super::Js). Values it hands out are handles like any other and
/// outlive the call.
pub struct Scope<'a> {
    pub(crate) core: NativeCx<'a>,
    pub(crate) shared: Rc<Shared>,
    call: NativeCall,
}

impl Scope<'_> {
    /// The operations' view of the engine for one operation.
    pub(crate) fn view(&mut self) -> View<'_> {
        View::new(self.core.reborrow(), &self.shared)
    }

    fn lift(&mut self, value: oxide_core::Value) -> Value {
        // Lifting fails only when a string cannot be copied out (the heap
        // cap), which a read of the call reports as `undefined`.
        self.view().lift(value).unwrap_or(Value::Undefined)
    }

    /// The receiver: `this` of the call.
    pub fn this(&mut self) -> Value {
        let this = self.call.this(&self.core);
        self.lift(this)
    }

    /// Argument `index`; `undefined` past [`Scope::argc`].
    pub fn arg(&mut self, index: usize) -> Value {
        let arg = self.call.arg(&self.core, index);
        self.lift(arg)
    }

    /// The number of arguments passed, whatever the function's `length`.
    pub fn argc(&self) -> usize {
        self.call.argc()
    }

    /// Every argument passed.
    pub fn args(&mut self) -> Vec<Value> {
        (0..self.argc()).map(|index| self.arg(index)).collect()
    }

    /// `new.target`; `undefined` for a call.
    pub fn new_target(&mut self) -> Value {
        let target = self.call.new_target();
        self.lift(target)
    }

    /// Whether `new` called the function.
    pub fn is_construct(&self) -> bool {
        self.call.is_construct()
    }

    /// The function object called.
    pub fn callee(&mut self) -> CallableRef {
        let callee = self.call.callee().as_object().expect("a host function");
        CallableRef(self.view().lift_object(callee))
    }

    /// Captured value `slot` ([`FunctionOptions::data`]); `undefined` past
    /// the last.
    pub fn data(&mut self, slot: usize) -> Value {
        if slot >= self.call.data_len(&self.core) {
            return Value::Undefined;
        }
        let value = self.call.data(&self.core, slot);
        self.lift(value)
    }

    /// Replaces captured value `slot`, which must exist.
    pub fn set_data(&mut self, slot: usize, value: Value) -> JsResult<()> {
        if slot >= self.call.data_len(&self.core) {
            return Err(RuntimeError::Invariant("a captured value's slot"));
        }
        let call = self.call;
        let mut view = self.view();
        let value = view.lower(&value)?;
        call.set_data(&mut view.core, slot, value);
        Ok(())
    }

    /// The value a host object receiver holds ([`Js::host_data`](super::Js::host_data) of
    /// `this`); `None` when the receiver is not a host object of `T`.
    pub fn this_host_data<T: 'static>(&mut self) -> JsResult<Option<Rc<T>>> {
        match self.this() {
            Value::Object(object) => self.host_data::<T>(&object),
            _ => Ok(None),
        }
    }
}

/// How a host function is made: its `name` and `length`, whether `new` may
/// call it, and the values it captures ([`Scope::data`]). Captured values are
/// traced by the collector; a Rust value the closure captures is not.
#[derive(Clone, Debug, Default)]
pub struct FunctionOptions {
    pub name: String,
    pub length: u32,
    pub constructor: bool,
    pub data: Vec<Value>,
}

impl FunctionOptions {
    pub fn new(name: impl Into<String>) -> Self {
        FunctionOptions {
            name: name.into(),
            ..FunctionOptions::default()
        }
    }

    #[must_use]
    pub fn length(mut self, length: u32) -> Self {
        self.length = length;
        self
    }

    /// The function may be called by `new` ([`Scope::is_construct`]). It
    /// has no `prototype` property until the embedder defines one.
    #[must_use]
    pub fn constructor(mut self) -> Self {
        self.constructor = true;
        self
    }

    #[must_use]
    pub fn data(mut self, data: Vec<Value>) -> Self {
        self.data = data;
        self
    }
}

/// What [`Js::new_function`](super::Js::new_function) takes: a closure over
/// a [`Scope`]. `Args` is the marker the closure's shape selects on.
pub trait HostFunction<Args>: 'static {
    fn call(&self, scope: &mut Scope<'_>) -> JsResult<Value>;
}

/// The marker of a closure over the [`Scope`] alone.
pub struct Raw;

impl<F> HostFunction<Raw> for F
where
    F: Fn(&mut Scope<'_>) -> JsResult<Value> + 'static,
{
    fn call(&self, scope: &mut Scope<'_>) -> JsResult<Value> {
        self(scope)
    }
}

/// Marks the runtime poisoned when a panic unwinds through a host function.
struct PoisonGuard<'a>(&'a Shared);

impl Drop for PoisonGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.poisoned.set(true);
        }
    }
}

/// The core's body for `function`: it builds the [`Scope`], calls the
/// embedder's closure and translates its result into a value or a throw.
/// The runtime is held weakly, since the body lives inside it.
pub(crate) fn host_body<Args>(
    shared: &Rc<Shared>,
    function: impl HostFunction<Args>,
) -> Rc<HostFn> {
    let weak: Weak<Shared> = Rc::downgrade(shared);
    Rc::new(move |core: &mut NativeCx<'_>, call: NativeCall| {
        let shared = weak
            .upgrade()
            .expect("a host function runs inside its live runtime");
        let _guard = PoisonGuard(&shared);
        let mut scope = Scope {
            core: core.reborrow(),
            shared: Rc::clone(&shared),
            call,
        };
        let result = function.call(&mut scope);
        drop(scope);
        let mut view = View::new(core.reborrow(), &shared);
        let completion = match result {
            // A constructor's non-object result passes through, as QuickJS
            // passes it.
            Ok(value) => view
                .lower(&value)
                .map_err(|error| error_throw(&mut view, error)),
            Err(error) => Err(error_throw(&mut view, error)),
        };
        drop(view);
        // Handles the closure dropped wait for nothing: a script that calls
        // a host function many times keeps only one call's handles.
        shared.drain_released(core);
        completion
    })
}

/// The throw a host function's error becomes.
fn error_throw(view: &mut View<'_>, error: RuntimeError) -> oxide_core::Throw {
    let thrown = match error {
        RuntimeError::Exception(value) => view.lower(&value),
        // A nested call was interrupted: the interrupt keeps unwinding, the
        // flag still set.
        RuntimeError::Interrupted => view
            .core
            .new_error(oxide_core::ErrorKind::Internal, "interrupted")
            .map_err(RuntimeError::from),
        other => Err(other),
    };
    match thrown {
        Ok(value) => oxide_core::Throw(value),
        Err(RuntimeError::OutOfMemory) => out_of_memory(view),
        Err(other) => {
            let message = other.to_string();
            match view
                .core
                .new_error(oxide_core::ErrorKind::Internal, &message)
            {
                Ok(error) => oxide_core::Throw(error),
                Err(_) => out_of_memory(view),
            }
        }
    }
}

fn out_of_memory(view: &View<'_>) -> oxide_core::Throw {
    let error = view.core.intrinsic(Intrinsic::OutOfMemoryError);
    oxide_core::Throw(oxide_core::Value::object(error))
}
