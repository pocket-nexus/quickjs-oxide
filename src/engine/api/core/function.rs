//! Functions the embedder defines in Rust: a closure registered with the
//! runtime and called through the core's host hook with a [`CallScope`].

use std::rc::Rc;

use oxide_core::Handle;
use oxide_core::builtins::host::HostCx;

use super::RuntimeError;
use super::context::{Context, native_kind};
use super::runtime::{Shared, current, slot};
use super::value::{CallableRef, ObjectRef, SymbolRef, Value};
use crate::engine::api::error::NativeErrorKind;
use crate::engine::value::JsString;
use crate::engine::value::bigint::JsBigInt;

/// A function of the embedder. An `Err` is the value to throw.
pub type HostFunction = dyn Fn(&mut CallScope<'_, '_, '_>) -> Result<Value, Value>;

/// A host function's view of the runtime for one call: its receiver and
/// arguments, and the operations it may run while the engine is borrowed.
/// Values made through it are handles like any other and outlive the call.
pub struct CallScope<'s, 'r, 'a> {
    host: &'s mut HostCx<'r, 'a>,
    shared: &'s Rc<Shared>,
    /// Handles for lowered arguments, released when the call returns.
    temps: Vec<Handle>,
}

impl CallScope<'_, '_, '_> {
    fn lift(&mut self, value: oxide_core::Value) -> Value {
        use oxide_core::value::Kind;
        match value.kind() {
            Kind::Undefined => Value::Undefined,
            Kind::Null => Value::Null,
            Kind::Bool => Value::Bool(value.as_bool().unwrap_or(false)),
            Kind::Int32 => Value::Int(value.as_int32().unwrap_or(0)),
            Kind::Number => Value::Float(value.as_f64().unwrap_or(f64::NAN)),
            Kind::String => {
                let units = self.host.string_units(value).unwrap_or_default();
                JsString::try_from_utf16(units).map_or(Value::Undefined, Value::String)
            }
            Kind::BigInt => self
                .host
                .bigint_decimal(value)
                .and_then(|decimal| JsBigInt::from_decimal_unchecked(&decimal))
                .map_or(Value::Undefined, Value::BigInt),
            Kind::Symbol => Value::Symbol(SymbolRef {
                slot: slot(self.shared, self.host.hold(value), value),
            }),
            Kind::Object => Value::Object(ObjectRef {
                slot: slot(self.shared, self.host.hold(value), value),
            }),
            _ => Value::Undefined,
        }
    }

    fn lift_object(&mut self, object: oxide_core::ObjectId) -> ObjectRef {
        let value = oxide_core::Value::object(object);
        ObjectRef {
            slot: slot(self.shared, self.host.hold(value), value),
        }
    }

    /// The core value of `value`, rooted until the call returns.
    fn lower(&mut self, value: &Value) -> Result<oxide_core::Value, Value> {
        let lowered = match value {
            Value::Undefined => oxide_core::Value::UNDEFINED,
            Value::Null => oxide_core::Value::NULL,
            Value::Bool(value) => oxide_core::Value::bool(*value),
            Value::Int(value) => oxide_core::Value::int32(*value),
            Value::Float(value) => oxide_core::Value::number(*value),
            Value::String(text) => {
                let units: Vec<u16> = text.utf16_units().collect();
                let value = self
                    .host
                    .new_string_units(&units)
                    .map_err(|e| self.lift(e))?;
                self.temp(value);
                value
            }
            Value::BigInt(_) => {
                return Err(self.error(NativeErrorKind::Type, "BigInt arguments are unsupported"));
            }
            Value::Symbol(symbol) => self.own(symbol.core_value(), &symbol.slot.shared)?,
            Value::Object(object) => self.own(object.core_value(), &object.slot.shared)?,
        };
        Ok(lowered)
    }

    fn own(
        &mut self,
        value: oxide_core::Value,
        shared: &Rc<Shared>,
    ) -> Result<oxide_core::Value, Value> {
        if Rc::ptr_eq(shared, self.shared) {
            Ok(value)
        } else {
            Err(self.error(NativeErrorKind::Type, "a value of another runtime"))
        }
    }

    fn object(&mut self, object: &ObjectRef) -> Result<oxide_core::ObjectId, Value> {
        let value = self.own(object.core_value(), &object.slot.shared)?;
        Ok(value.as_object().expect("an object reference"))
    }

    fn temp(&mut self, value: oxide_core::Value) {
        let handle = self.host.hold(value);
        self.temps.push(handle);
    }

    fn key(&mut self, name: &str) -> Result<oxide_core::PropertyKey, Value> {
        let units: Vec<u16> = name.encode_utf16().collect();
        let key = match self.host.key_from_units(&units) {
            Ok(key) => key,
            Err(_) => return Err(self.error(NativeErrorKind::Internal, "out of memory")),
        };
        if let Some(value) = key.heap_value() {
            self.temp(value);
        }
        Ok(key)
    }

    // ---- the call ----

    /// The receiver.
    pub fn this(&mut self) -> Value {
        let this = self.host.this();
        self.lift(this)
    }

    /// The number of arguments passed.
    pub fn argc(&self) -> usize {
        self.host.argc()
    }

    /// Argument `index`; `undefined` past the last.
    pub fn arg(&mut self, index: usize) -> Value {
        let value = self.host.arg(index);
        self.lift(value)
    }

    /// Every argument passed.
    pub fn args(&mut self) -> Vec<Value> {
        (0..self.argc()).map(|index| self.arg(index)).collect()
    }

    /// `new.target` when `new` called the function.
    pub fn new_target(&mut self) -> Option<Value> {
        let target = self.host.new_target()?;
        Some(self.lift(target))
    }

    // ---- objects ----

    pub fn global_object(&mut self) -> ObjectRef {
        let global = self.host.global();
        self.lift_object(global)
    }

    /// A new ordinary object.
    pub fn new_object(&mut self) -> Result<ObjectRef, Value> {
        let prototype = self.host.intrinsic(oxide_core::Intrinsic::ObjectPrototype);
        self.new_object_with_prototype_id(Some(prototype))
    }

    /// A new ordinary object inheriting from `prototype` (`None` for null).
    pub fn new_object_with_prototype(
        &mut self,
        prototype: Option<&ObjectRef>,
    ) -> Result<ObjectRef, Value> {
        let prototype = prototype.map(|object| self.object(object)).transpose()?;
        self.new_object_with_prototype_id(prototype)
    }

    fn new_object_with_prototype_id(
        &mut self,
        prototype: Option<oxide_core::ObjectId>,
    ) -> Result<ObjectRef, Value> {
        match self.host.new_object_with_prototype(prototype) {
            Ok(object) => Ok(self.lift_object(object)),
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    /// An array of `values`.
    pub fn new_array(&mut self, values: &[Value]) -> Result<ObjectRef, Value> {
        let lowered = values
            .iter()
            .map(|value| self.lower(value))
            .collect::<Result<Vec<_>, _>>()?;
        match self.host.new_array_from(&lowered) {
            Ok(array) => Ok(self.lift_object(array)),
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    /// `object[name]`.
    pub fn get(&mut self, object: &ObjectRef, name: &str) -> Result<Value, Value> {
        let id = self.object(object)?;
        let key = self.key(name)?;
        match self.host.get(id, key) {
            Ok(value) => Ok(self.lift(value)),
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    /// `object[name] = value`.
    pub fn set(&mut self, object: &ObjectRef, name: &str, value: &Value) -> Result<(), Value> {
        let id = self.object(object)?;
        let key = self.key(name)?;
        let value = self.lower(value)?;
        match self.host.set(id, key, value) {
            Ok(_) => Ok(()),
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    /// `CreateDataPropertyOrThrow(object, name, value)`.
    pub fn define(&mut self, object: &ObjectRef, name: &str, value: &Value) -> Result<(), Value> {
        let id = self.object(object)?;
        let key = self.key(name)?;
        let value = self.lower(value)?;
        self.host
            .create_data_property(id, key, value)
            .map_err(|thrown| self.lift(thrown))
    }

    /// A function of the embedder (see [`Context::new_function`]).
    pub fn new_function(
        &mut self,
        name: &str,
        length: u32,
        function: impl Fn(&mut CallScope<'_, '_, '_>) -> Result<Value, Value> + 'static,
    ) -> Result<CallableRef, Value> {
        let id = register(self.shared, Rc::new(function));
        match self.host.new_host_function(id, name, length, false) {
            Ok(object) => Ok(CallableRef(self.lift_object(object))),
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    // ---- bytes ----

    /// The core value of `value` when it is an object of this runtime.
    fn own_object(&self, value: &Value) -> Option<oxide_core::Value> {
        match value {
            Value::Object(object) if Rc::ptr_eq(&object.slot.shared, self.shared) => {
                Some(object.core_value())
            }
            _ => None,
        }
    }

    /// The bytes `value` exposes (see [`CallScope::bytes_mut`]).
    pub fn bytes(&self, value: &Value) -> Option<&[u8]> {
        let value = self.own_object(value)?;
        self.host.bytes(value)
    }

    /// The bytes `value` exposes, writable: an `ArrayBuffer`'s, or a typed
    /// array's or `DataView`'s window on its buffer. `None` for any other
    /// value, a `SharedArrayBuffer` or a view on one, and a detached buffer
    /// or out-of-bounds view. The scope is borrowed while the bytes are, so
    /// no script runs and the buffer cannot change meanwhile.
    pub fn bytes_mut(&mut self, value: &Value) -> Option<&mut [u8]> {
        let value = self.own_object(value)?;
        self.host.bytes_mut(value)
    }

    /// A new `ArrayBuffer` owning `bytes`, without copying them.
    pub fn new_array_buffer(&mut self, bytes: Vec<u8>) -> Result<ObjectRef, Value> {
        match self.host.new_array_buffer(bytes) {
            Ok(buffer) => Ok(self.lift_object(buffer)),
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    // ---- calls ----

    /// `Call(function, this, args)`.
    pub fn call(&mut self, function: &Value, this: &Value, args: &[Value]) -> Result<Value, Value> {
        let function = self.lower(function)?;
        let this = self.lower(this)?;
        let args = args
            .iter()
            .map(|value| self.lower(value))
            .collect::<Result<Vec<_>, _>>()?;
        match self.host.call(function, this, &args) {
            Ok(value) => Ok(self.lift(value)),
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    pub fn is_callable(&mut self, value: &Value) -> bool {
        match value {
            Value::Object(object) => {
                Rc::ptr_eq(&object.slot.shared, self.shared)
                    && self.host.is_callable(object.core_value())
            }
            _ => false,
        }
    }

    // ---- conversions and errors ----

    /// `ToString(value)` as Rust text (lone surrogates replaced).
    pub fn to_string(&mut self, value: &Value) -> Result<String, Value> {
        if let Value::String(text) = value {
            return Ok(String::from_utf16_lossy(
                &text.utf16_units().collect::<Vec<_>>(),
            ));
        }
        let lowered = self.lower(value)?;
        match self.host.to_string(lowered) {
            Ok(text) => {
                let units = self.host.string_units(text).unwrap_or_default();
                Ok(String::from_utf16_lossy(&units))
            }
            Err(thrown) => Err(self.lift(thrown)),
        }
    }

    /// `ToNumber(value)`.
    pub fn to_number(&mut self, value: &Value) -> Result<f64, Value> {
        let lowered = self.lower(value)?;
        self.host
            .to_number(lowered)
            .map_err(|thrown| self.lift(thrown))
    }

    /// A new error object of `kind` with `message`, to throw.
    pub fn error(&mut self, kind: NativeErrorKind, message: &str) -> Value {
        let error = self.host.new_error(native_kind(kind), message);
        self.lift(error)
    }

    /// A new `TypeError` with `message`, to throw.
    pub fn type_error(&mut self, message: &str) -> Value {
        self.error(NativeErrorKind::Type, message)
    }
}

/// Adds `function` to the runtime's table; its index.
fn register(shared: &Rc<Shared>, function: Rc<HostFunction>) -> u32 {
    let mut functions = shared.functions.borrow_mut();
    functions.push(function);
    (functions.len() - 1) as u32
}

/// The core's host hook: runs function `id` of the innermost runtime.
pub(crate) fn host_call_shim(
    host: &mut HostCx<'_, '_>,
    id: u32,
) -> Result<oxide_core::Value, oxide_core::Value> {
    let shared = current().expect("a host function runs inside an entry");
    let function = shared.functions.borrow().get(id as usize).cloned();
    let Some(function) = function else {
        return Err(host.new_error(oxide_core::ErrorKind::Internal, "no such host function"));
    };
    let mut scope = CallScope {
        host,
        shared: &shared,
        temps: Vec::new(),
    };
    let result = function(&mut scope);
    // The result is lowered unrooted: releasing the temporaries does not
    // allocate, and the loop stores it before its next allocation.
    // A value that cannot be lowered throws the error lowering made.
    let lowered = |scope: &mut CallScope<'_, '_, '_>, value: &Value| {
        scope.lower(value).map_err(|error| match error {
            Value::Object(object) => object.core_value(),
            _ => oxide_core::Value::UNDEFINED,
        })
    };
    let result = match result {
        Ok(value) => lowered(&mut scope, &value),
        Err(thrown) => Err(lowered(&mut scope, &thrown).unwrap_or_else(|error| error)),
    };
    let temps = std::mem::take(&mut scope.temps);
    for handle in temps {
        scope.host.release(handle);
    }
    result
}

impl Context {
    /// A function named `name` with `length` that runs `function`: its
    /// receiver and arguments are the [`CallScope`]'s, and an `Err` throws.
    /// The closure lives as long as the runtime.
    pub fn new_function(
        &mut self,
        name: &str,
        length: u32,
        function: impl Fn(&mut CallScope<'_, '_, '_>) -> Result<Value, Value> + 'static,
    ) -> Result<CallableRef, RuntimeError> {
        self.new_host_function(name, length, false, Rc::new(function))
    }

    /// [`Context::new_function`] for a function `new` may call:
    /// [`CallScope::new_target`] tells a construction from a call.
    pub fn new_constructor(
        &mut self,
        name: &str,
        length: u32,
        function: impl Fn(&mut CallScope<'_, '_, '_>) -> Result<Value, Value> + 'static,
    ) -> Result<CallableRef, RuntimeError> {
        self.new_host_function(name, length, true, Rc::new(function))
    }

    fn new_host_function(
        &mut self,
        name: &str,
        length: u32,
        constructor: bool,
        function: Rc<HostFunction>,
    ) -> Result<CallableRef, RuntimeError> {
        let id = register(&self.runtime.shared, function);
        let mut entry = self.runtime.enter()?;
        let result = entry
            .inner
            .core
            .new_host_function(id, name, length, constructor);
        let object = entry.complete_with(result)?;
        Ok(CallableRef(entry.lift_object(object)))
    }
}
