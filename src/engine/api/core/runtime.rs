//! The API's runtime (phase 6, section 2.1): one `Rc<RefCell<…>>` at the
//! boundary around `oxide_core::Runtime`. Every method borrows the cell for
//! one engine entry; a host callback that calls back in without its
//! [`Scope`](super::Scope) gets [`RuntimeError::Reentered`].

use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;

use oxide_core::Handle;
use oxide_core::host::Host;
use oxide_core::vm::module::ModuleTables;
use oxide_core::{Intrinsic, MemoryReport, RuntimeOptions};

use super::RuntimeError;
use super::context::Context;
use super::module::ModuleLoader;
use super::property::{
    AccessorValue, CompleteOrdinaryPropertyDescriptor, DescriptorField, Key,
    OrdinaryPropertyDescriptor, PropertyKey, WellKnownSymbol,
};
use super::value::{CallableRef, ObjectRef, Slot, SymbolRef, Value};
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::value::JsString;
use crate::engine::value::bigint::JsBigInt;

/// A runtime: one heap, one realm, one job queue. Cloning shares it.
#[derive(Clone)]
pub struct Runtime {
    pub(crate) shared: Rc<Shared>,
}

/// A module compiled by `Context::compile_module*`, waiting for the core to
/// load it by name.
pub(crate) struct Preloaded {
    pub(crate) function: ObjectRef,
    pub(crate) tables: ModuleTables,
    pub(crate) import_meta: Vec<(String, Value)>,
}

pub(crate) struct Shared {
    pub(crate) inner: RefCell<Inner>,
    /// Handles whose last reference dropped while the runtime was borrowed;
    /// released by the next entry.
    released: RefCell<Vec<Handle>>,
    pub(crate) loader: RefCell<Option<Box<dyn ModuleLoader>>>,
    pub(crate) modules: RefCell<HashMap<String, Preloaded>>,
    pub(crate) debug_info: Cell<DebugInfoMode>,
    /// Execution turns in progress (`with_execution_turn`).
    turns: Cell<u32>,
}

impl Shared {
    /// Releases `handle` now, or at the next entry when the runtime is
    /// borrowed (a value dropped inside a host callback). Never fails.
    pub(crate) fn release(&self, handle: Handle) {
        match self.inner.try_borrow_mut() {
            Ok(mut inner) => inner.core.release(handle),
            Err(_) => self.released.borrow_mut().push(handle),
        }
    }

    fn drain_released(&self, inner: &mut Inner) {
        let released = std::mem::take(&mut *self.released.borrow_mut());
        for handle in released {
            inner.core.release(handle);
        }
    }
}

pub(crate) struct Inner {
    pub(crate) core: oxide_core::Runtime,
    /// The pending exception (`RuntimeError::Exception`).
    pub(crate) exception: Option<Value>,
    /// Handles an entry took for converted arguments; released when the
    /// entry ends.
    temps: Vec<Handle>,
}

thread_local! {
    /// The runtimes with an entry in progress on this thread, innermost
    /// last: how the core's module and compile callbacks find the API.
    static CURRENT: RefCell<Vec<Rc<Shared>>> = const { RefCell::new(Vec::new()) };
}

/// The runtime of the innermost entry on this thread.
pub(crate) fn current() -> Option<Rc<Shared>> {
    CURRENT.with(|current| current.borrow().last().cloned())
}

pub(crate) fn current_debug_info() -> Option<DebugInfoMode> {
    current().map(|shared| shared.debug_info.get())
}

/// One engine entry: the borrowed runtime.
pub(crate) struct Entry<'a> {
    pub(crate) inner: RefMut<'a, Inner>,
    pub(crate) shared: &'a Rc<Shared>,
}

impl Drop for Entry<'_> {
    fn drop(&mut self) {
        CURRENT.with(|current| current.borrow_mut().pop());
        let temps = std::mem::take(&mut self.inner.temps);
        for handle in temps {
            self.inner.core.release(handle);
        }
    }
}

impl std::ops::Deref for Entry<'_> {
    type Target = Inner;

    fn deref(&self) -> &Inner {
        &self.inner
    }
}

impl std::ops::DerefMut for Entry<'_> {
    fn deref_mut(&mut self) -> &mut Inner {
        &mut self.inner
    }
}

/// Makes a handle-owning reference from a held handle.
pub(crate) fn slot(shared: &Rc<Shared>, handle: Handle, value: oxide_core::Value) -> Rc<Slot> {
    Rc::new(Slot {
        shared: Rc::clone(shared),
        handle,
        value,
    })
}

impl Entry<'_> {
    /// Copies a core value out: primitives as data, objects and symbols as
    /// new handles. `value` must still be live (just returned or rooted).
    pub(crate) fn lift(&mut self, value: oxide_core::Value) -> Result<Value, RuntimeError> {
        use oxide_core::value::Kind;
        Ok(match value.kind() {
            Kind::Undefined => Value::Undefined,
            Kind::Null => Value::Null,
            Kind::Bool => Value::Bool(value.as_bool().unwrap_or(false)),
            Kind::Int32 => Value::Int(value.as_int32().unwrap_or(0)),
            Kind::Number => Value::Float(value.as_f64().unwrap_or(f64::NAN)),
            Kind::String => {
                let units = self.core.string_units(value).unwrap_or_default();
                Value::String(JsString::try_from_utf16(units)?)
            }
            Kind::BigInt => {
                let decimal = self.core.bigint_decimal(value).unwrap_or_default();
                Value::BigInt(
                    JsBigInt::parse_js_string(&decimal)
                        .map_err(|error| RuntimeError::Internal(format!("{error:?}")))?,
                )
            }
            Kind::Symbol => Value::Symbol(SymbolRef {
                slot: slot(self.shared, self.inner.core.hold(value), value),
            }),
            Kind::Object => Value::Object(ObjectRef {
                slot: slot(self.shared, self.inner.core.hold(value), value),
            }),
            _ => Value::Undefined,
        })
    }

    pub(crate) fn lift_object(&mut self, object: oxide_core::ObjectId) -> ObjectRef {
        let value = oxide_core::Value::object(object);
        ObjectRef {
            slot: slot(self.shared, self.inner.core.hold(value), value),
        }
    }

    /// The core value of `value`, rooted until the entry ends.
    pub(crate) fn lower(&mut self, value: &Value) -> Result<oxide_core::Value, RuntimeError> {
        let lowered = match value {
            Value::Undefined => oxide_core::Value::UNDEFINED,
            Value::Null => oxide_core::Value::NULL,
            Value::Bool(value) => oxide_core::Value::bool(*value),
            Value::Int(value) => oxide_core::Value::int32(*value),
            Value::Float(value) => oxide_core::Value::number(*value),
            Value::String(text) => {
                let units: Vec<u16> = text.utf16_units().collect();
                let value = self.inner.core.new_string_units(&units)?;
                self.temp(value);
                value
            }
            Value::BigInt(big) => {
                let decimal = big
                    .to_string_radix(10)
                    .map_err(|error| RuntimeError::Internal(format!("{error:?}")))?;
                let value = self
                    .inner
                    .core
                    .new_bigint(&decimal)
                    .ok_or(RuntimeError::Invariant("a BigInt's decimal text"))??;
                self.temp(value);
                value
            }
            Value::Symbol(symbol) => self.own(symbol.slot.as_ref())?,
            Value::Object(object) => self.own(object.slot.as_ref())?,
        };
        Ok(lowered)
    }

    fn own(&self, slot: &Slot) -> Result<oxide_core::Value, RuntimeError> {
        if Rc::ptr_eq(&slot.shared, self.shared) {
            Ok(slot.value)
        } else {
            Err(RuntimeError::Invariant("a value of another runtime"))
        }
    }

    pub(crate) fn object(&self, object: &ObjectRef) -> Result<oxide_core::ObjectId, RuntimeError> {
        self.own(&object.slot)?
            .as_object()
            .ok_or(RuntimeError::Invariant("an object reference"))
    }

    fn temp(&mut self, value: oxide_core::Value) {
        let handle = self.inner.core.hold(value);
        self.inner.temps.push(handle);
    }

    /// The core key of `key`, rooted until the entry ends.
    pub(crate) fn key(
        &mut self,
        key: &PropertyKey,
    ) -> Result<oxide_core::PropertyKey, RuntimeError> {
        match &key.0 {
            Key::Index(index) => Ok(oxide_core::PropertyKey::index(*index).map_or_else(
                || {
                    let units: Vec<u16> = index.to_string().encode_utf16().collect();
                    self.inner.core.key_from_units(&units)
                },
                Ok,
            )?),
            Key::String(text) => {
                let units: Vec<u16> = text.utf16_units().collect();
                Ok(self.inner.core.key_from_units(&units)?)
            }
            Key::Symbol(symbol) => {
                let value = self.own(&symbol.slot)?;
                self.inner
                    .core
                    .key_from_symbol(value)
                    .ok_or(RuntimeError::Invariant("a symbol reference"))
            }
        }
    }

    /// The API key of a core key.
    pub(crate) fn lift_key(
        &mut self,
        key: oxide_core::PropertyKey,
    ) -> Result<PropertyKey, RuntimeError> {
        Ok(match self.inner.core.key_name(key) {
            oxide_core::KeyName::Index(index) => PropertyKey(Key::Index(index)),
            oxide_core::KeyName::String(units) => {
                PropertyKey(Key::String(JsString::try_from_utf16(units)?))
            }
            oxide_core::KeyName::Symbol(symbol) => match self.lift(symbol)? {
                Value::Symbol(symbol) => PropertyKey(Key::Symbol(symbol)),
                _ => return Err(RuntimeError::Invariant("a symbol key")),
            },
        })
    }

    /// Records a throw as the pending exception; the error to return.
    pub(crate) fn thrown(&mut self, thrown: oxide_core::Throw) -> RuntimeError {
        let out_of_memory = self.inner.core.intrinsic(Intrinsic::OutOfMemoryError);
        if thrown.0.as_object() == Some(out_of_memory) {
            return RuntimeError::OutOfMemory;
        }
        match self.lift(thrown.0) {
            Ok(value) => {
                self.inner.exception = Some(value);
                RuntimeError::Exception
            }
            Err(error) => error,
        }
    }

    /// Converts a core result, recording a throw.
    pub(crate) fn complete(
        &mut self,
        result: Result<oxide_core::Value, oxide_core::Throw>,
    ) -> Result<Value, RuntimeError> {
        match result {
            Ok(value) => self.lift(value),
            Err(thrown) => Err(self.thrown(thrown)),
        }
    }

    pub(crate) fn complete_with<T>(
        &mut self,
        result: Result<T, oxide_core::Throw>,
    ) -> Result<T, RuntimeError> {
        result.map_err(|thrown| self.thrown(thrown))
    }

    /// A descriptor in the core's form, its values rooted until the entry
    /// ends. Accessor fields that are not callable were rejected by the
    /// type already.
    pub(crate) fn lower_descriptor(
        &mut self,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<oxide_core::PropertyDescriptor, RuntimeError> {
        let accessor = |entry: &mut Self, field: &DescriptorField<AccessorValue>| match field {
            DescriptorField::Absent => Ok(None),
            DescriptorField::Present(AccessorValue::Undefined) => {
                Ok(Some(oxide_core::Value::UNDEFINED))
            }
            DescriptorField::Present(AccessorValue::Callable(function)) => {
                entry.own(&function.0.slot).map(Some)
            }
        };
        Ok(oxide_core::PropertyDescriptor {
            value: match &descriptor.value {
                DescriptorField::Absent => None,
                DescriptorField::Present(value) => Some(self.lower(value)?),
            },
            get: accessor(self, &descriptor.get)?,
            set: accessor(self, &descriptor.set)?,
            writable: descriptor.writable.into_option(),
            enumerable: descriptor.enumerable.into_option(),
            configurable: descriptor.configurable.into_option(),
        })
    }

    pub(crate) fn lift_descriptor(
        &mut self,
        descriptor: oxide_core::PropertyDescriptor,
    ) -> Result<CompleteOrdinaryPropertyDescriptor, RuntimeError> {
        if descriptor.is_accessor() {
            let callable = |entry: &mut Self, value: Option<oxide_core::Value>| {
                Ok::<_, RuntimeError>(match value {
                    Some(value) if !value.is_undefined() => match entry.lift(value)? {
                        Value::Object(object) => Some(CallableRef(object)),
                        _ => None,
                    },
                    _ => None,
                })
            };
            Ok(CompleteOrdinaryPropertyDescriptor::Accessor {
                get: callable(self, descriptor.get)?,
                set: callable(self, descriptor.set)?,
                enumerable: descriptor.enumerable.unwrap_or(false),
                configurable: descriptor.configurable.unwrap_or(false),
            })
        } else {
            let value = descriptor.value.unwrap_or(oxide_core::Value::UNDEFINED);
            Ok(CompleteOrdinaryPropertyDescriptor::Data {
                value: self.lift(value)?,
                writable: descriptor.writable.unwrap_or(false),
                enumerable: descriptor.enumerable.unwrap_or(false),
                configurable: descriptor.configurable.unwrap_or(false),
            })
        }
    }
}

/// What `run_pending_job` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PendingJobOutcome {
    /// The queue was empty.
    NoJob,
    /// One job ran.
    Executed,
}

impl PendingJobOutcome {
    pub const fn executed(self) -> bool {
        matches!(self, Self::Executed)
    }
}

/// A promise's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromiseState {
    Pending,
    Fulfilled,
    Rejected,
}

/// A promise's state and result, read without running JavaScript.
#[derive(Clone, Debug, PartialEq)]
pub struct PromiseSnapshot {
    state: PromiseState,
    result: Value,
}

impl PromiseSnapshot {
    pub fn try_clone(&self) -> Result<Self, RuntimeError> {
        Ok(self.clone())
    }

    pub const fn state(&self) -> PromiseState {
        self.state
    }

    /// The fulfillment value or rejection reason; `undefined` while pending.
    pub const fn result(&self) -> &Value {
        &self.result
    }
}

/// When the collector runs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum GcPolicy {
    /// When the heap has grown by the growth factor since the last
    /// collection (and at the cap).
    #[default]
    Automatic,
    /// Only at the cap and on `Runtime::run_gc`.
    Manual,
}

/// The collector's statistics.
pub use oxide_core::heap::gc::GcStats;

/// A default host over the process: its streams, the system clock, UTC.
pub use oxide_core::host::SystemHost;

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Runtime {
    /// A runtime with the default host and options.
    pub fn new() -> Self {
        Self::with_options(RuntimeOptions::default())
    }

    pub fn with_options(options: RuntimeOptions) -> Self {
        Self::with_host(SystemHost, options)
    }

    /// A runtime over `host` (the clock, the time zone, output, files).
    pub fn with_host(host: impl Host + 'static, options: RuntimeOptions) -> Self {
        let mut core = oxide_core::Runtime::new(options)
            .expect("the initial realm fits any heap cap the options allow");
        core.set_host(Box::new(host));
        core.set_compiler(super::compile::compile_for_core);
        core.set_module_host(oxide_core::vm::module::ModuleHost {
            normalize: super::module::normalize_shim,
            load: super::module::load_shim,
        });
        Runtime {
            shared: Rc::new(Shared {
                inner: RefCell::new(Inner {
                    core,
                    exception: None,
                    temps: Vec::new(),
                }),
                released: RefCell::new(Vec::new()),
                loader: RefCell::new(None),
                modules: RefCell::new(HashMap::new()),
                debug_info: Cell::new(DebugInfoMode::Full),
                turns: Cell::new(0),
            }),
        }
    }

    /// [`Runtime::with_host`] with the default options (the current API's
    /// name).
    pub fn new_with_host_services(host: impl Host + 'static) -> Self {
        Self::with_host(host, RuntimeOptions::default())
    }

    /// Starts one engine entry.
    pub(crate) fn enter(&self) -> Result<Entry<'_>, RuntimeError> {
        let mut inner = self
            .shared
            .inner
            .try_borrow_mut()
            .map_err(|_| RuntimeError::Reentered)?;
        self.shared.drain_released(&mut inner);
        CURRENT.with(|current| current.borrow_mut().push(Rc::clone(&self.shared)));
        Ok(Entry {
            inner,
            shared: &self.shared,
        })
    }

    /// The runtime's one realm.
    pub fn context(&self) -> Context {
        Context {
            runtime: self.clone(),
        }
    }

    /// The runtime's one realm (the current API's name: a second call
    /// returns the same realm, never a new one).
    pub fn new_context(&self) -> Result<Context, RuntimeError> {
        Ok(self.context())
    }

    /// Whether `self` and `other` are the same runtime.
    pub fn is_same_runtime(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.shared, &other.shared)
    }

    /// Whether `Atomics.wait` may block.
    pub fn set_can_block(&self, can_block: bool) -> Result<(), RuntimeError> {
        self.enter()?.core.set_can_block(can_block);
        Ok(())
    }

    /// The debug information functions compiled from now on keep.
    pub fn set_debug_info_mode(&self, mode: DebugInfoMode) -> Result<(), RuntimeError> {
        self.shared.debug_info.set(mode);
        Ok(())
    }

    pub fn debug_info_mode(&self) -> Result<DebugInfoMode, RuntimeError> {
        Ok(self.shared.debug_info.get())
    }

    /// Installs the module loader; `import` and `import()` go through it.
    pub fn set_module_loader(
        &self,
        loader: impl ModuleLoader + 'static,
    ) -> Result<(), RuntimeError> {
        let mut slot = self
            .shared
            .loader
            .try_borrow_mut()
            .map_err(|_| RuntimeError::Reentered)?;
        *slot = Some(Box::new(loader));
        Ok(())
    }

    // ---- jobs ----

    /// Whether a job is pending.
    pub fn is_job_pending(&self) -> Result<bool, RuntimeError> {
        Ok(self.enter()?.core.has_pending_jobs())
    }

    /// Runs the oldest pending job. A job that throws leaves its value as
    /// the pending exception and returns `RuntimeError::Exception`; the
    /// queue keeps the jobs after it. Rejected inside an execution turn,
    /// leaving the job queued.
    pub fn run_pending_job(&self) -> Result<PendingJobOutcome, RuntimeError> {
        if self.shared.turns.get() > 0 {
            return Err(RuntimeError::Invariant(
                "a job was dispatched inside an execution turn",
            ));
        }
        let mut entry = self.enter()?;
        let result = entry.inner.core.run_pending_job();
        match entry.complete_with(result)? {
            true => Ok(PendingJobOutcome::Executed),
            false => Ok(PendingJobOutcome::NoJob),
        }
    }

    /// [`Runtime::run_pending_job`] (the current API's name).
    pub fn execute_pending_job(&self) -> Result<PendingJobOutcome, RuntimeError> {
        self.run_pending_job()
    }

    /// Runs jobs until the queue is empty or one throws.
    pub fn run_jobs(&self) -> Result<(), RuntimeError> {
        while self.run_pending_job()?.executed() {}
        Ok(())
    }

    /// Runs `body` as one synchronous execution sequence: objects a
    /// `WeakRef` keeps alive stay alive until the turn ends
    /// (ECMASCRIPT-WEAKREF-LIFETIME-001).
    pub fn with_execution_turn<T>(
        &self,
        body: impl FnOnce() -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        self.shared.turns.set(self.shared.turns.get() + 1);
        let result = body();
        let turns = self.shared.turns.get() - 1;
        self.shared.turns.set(turns);
        if turns == 0 {
            self.enter()?.core.clear_kept_objects();
        }
        result
    }

    /// A promise's state and result; `None` when `promise` is not one.
    pub fn promise_snapshot(
        &self,
        promise: &ObjectRef,
    ) -> Result<Option<PromiseSnapshot>, RuntimeError> {
        let mut entry = self.enter()?;
        let value = oxide_core::Value::object(entry.object(promise)?);
        let Some((state, result)) = entry.core.promise_state(value) else {
            return Ok(None);
        };
        let state = match state {
            1 => PromiseState::Fulfilled,
            2 => PromiseState::Rejected,
            _ => PromiseState::Pending,
        };
        let result = if state == PromiseState::Pending {
            Value::Undefined
        } else {
            entry.lift(result)?
        };
        Ok(Some(PromiseSnapshot { state, result }))
    }

    /// Keeps every rejected promise without a handler until one is attached
    /// (QuickJS's `js_std_promise_rejection_tracker`).
    pub fn set_track_rejections(&self, track: bool) -> Result<(), RuntimeError> {
        self.enter()?.core.set_track_rejections(track);
        Ok(())
    }

    /// The tracked unhandled rejections, promise and reason, in rejection
    /// order; the list is emptied.
    pub fn take_unhandled_rejections(&self) -> Result<Vec<(ObjectRef, Value)>, RuntimeError> {
        let mut entry = self.enter()?;
        let promises = entry.core.unhandled_rejections().to_vec();
        let mut rejections = Vec::with_capacity(promises.len());
        for promise in promises {
            let reason = entry
                .core
                .promise_state(promise)
                .map_or(oxide_core::Value::UNDEFINED, |(_, reason)| reason);
            let promise = entry.lift(promise)?;
            let reason = entry.lift(reason)?;
            if let Value::Object(promise) = promise {
                rejections.push((promise, reason));
            }
        }
        entry.core.clear_unhandled_rejections();
        Ok(rejections)
    }

    // ---- memory ----

    /// Runs a full collection; the collector's statistics after it.
    pub fn run_gc(&self) -> Result<GcStats, RuntimeError> {
        let mut entry = self.enter()?;
        entry.core.run_gc();
        Ok(entry.core.heap().gc().stats)
    }

    pub fn gc_stats(&self) -> Result<GcStats, RuntimeError> {
        Ok(self.enter()?.core.heap().gc().stats)
    }

    /// What every part of the runtime holds, without a collection.
    pub fn memory_report(&self) -> Result<MemoryReport, RuntimeError> {
        Ok(self.enter()?.core.memory_report())
    }

    /// When the collector runs: on heap growth (the default) or only at the
    /// cap and on [`Runtime::run_gc`].
    pub fn set_gc_policy(&self, policy: GcPolicy) -> Result<(), RuntimeError> {
        self.enter()?
            .core
            .heap_mut()
            .gc_mut()
            .set_manual(policy == GcPolicy::Manual);
        Ok(())
    }

    pub fn gc_policy(&self) -> Result<GcPolicy, RuntimeError> {
        Ok(if self.enter()?.core.heap().gc().is_manual() {
            GcPolicy::Manual
        } else {
            GcPolicy::Automatic
        })
    }

    /// Collects before every `interval`-th allocating operation (`qjs
    /// --gc-stress=N`), or stops (`None`); only in a build with the stress
    /// collector ([`Runtime::GC_STRESS_AVAILABLE`]).
    pub fn set_gc_stress(&self, interval: Option<u32>) -> Result<(), RuntimeError> {
        let mut entry = self.enter()?;
        #[cfg(feature = "gc-stress")]
        {
            let gc = entry.core.heap_mut().gc_mut();
            gc.stress = interval.is_some_and(|interval| interval > 0);
            gc.stress_interval = interval.unwrap_or(1).max(1);
            Ok(())
        }
        #[cfg(not(feature = "gc-stress"))]
        {
            let _ = &mut entry;
            match interval {
                Some(_) => Err(RuntimeError::Unsupported(
                    "stress collections in this build",
                )),
                None => Ok(()),
            }
        }
    }

    /// Whether this build has the stress collector.
    pub const GC_STRESS_AVAILABLE: bool = cfg!(feature = "gc-stress");

    /// Whether this build counts inline-cache hits (`ic_report` returns
    /// `Some`).
    pub const IC_STATS_AVAILABLE: bool = cfg!(feature = "ic-stats");

    /// Records every collection's pause (for [`Runtime::gc_report`]).
    pub fn set_record_gc_pauses(&self, record: bool) -> Result<(), RuntimeError> {
        self.enter()?.core.heap_mut().gc_mut().record_pauses = record;
        Ok(())
    }

    /// `--gc-stats`'s line: collections, pauses and their percentiles, live
    /// and reserved bytes.
    pub fn gc_report(&self) -> Result<String, RuntimeError> {
        let entry = self.enter()?;
        let gc = entry.core.heap().gc();
        let stats = gc.stats;
        let mut pauses = gc.pauses.clone();
        pauses.sort();
        let percentile = |p: usize| {
            pauses
                .get((pauses.len() * p / 100).min(pauses.len().saturating_sub(1)))
                .map_or(0.0, |pause| pause.as_secs_f64() * 1e3)
        };
        Ok(format!(
            "gc: {} collections, total pause {:.1} ms, p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms, \
             live {} KiB, reserved {} KiB\n",
            stats.collections,
            stats.total_pause.as_secs_f64() * 1e3,
            percentile(50),
            percentile(99),
            stats.max_pause.as_secs_f64() * 1e3,
            stats.live_bytes / 1024,
            entry.core.heap().bytes() / 1024,
        ))
    }

    /// The inline-cache report (`--ic-stats`), when the build counts.
    pub fn ic_report(&self, worst: usize) -> Result<Option<String>, RuntimeError> {
        Ok(self.enter()?.core.ic_report(worst))
    }

    /// The disassembly of every unit `source` compiles to, root first.
    pub fn disassemble(&self, source: &[u8], filename: &str) -> Result<String, RuntimeError> {
        let mut entry = self.enter()?;
        let debug_info = self.shared.debug_info.get();
        let lowered = {
            let mut compiler = entry.inner.core.compiler();
            super::compile::compile_script(&mut compiler, source, filename, debug_info)
        };
        let lowered = match lowered {
            Ok(lowered) => lowered,
            Err(super::compile::ScriptCompileError::Front(error)) => {
                return Err(RuntimeError::Compile(error));
            }
            Err(super::compile::ScriptCompileError::Lower(error)) => {
                return Err(RuntimeError::Internal(error.to_string()));
            }
        };
        let mut text = String::new();
        for unit in entry.core.code().tree(lowered.root) {
            text.push_str(&entry.core.disassemble(unit));
            text.push('\n');
        }
        Ok(text)
    }

    // ---- keys and symbols ----

    pub fn intern_property_key(&self, text: &str) -> Result<PropertyKey, RuntimeError> {
        Ok(PropertyKey::from_js_string(JsString::try_from_utf8(text)?))
    }

    pub fn intern_property_key_js_string(
        &self,
        text: &JsString,
    ) -> Result<PropertyKey, RuntimeError> {
        Ok(PropertyKey::from_js_string(text.clone()))
    }

    /// The key as a string; a symbol key's description (empty when it has
    /// none).
    pub fn property_key_to_js_string(&self, key: &PropertyKey) -> Result<JsString, RuntimeError> {
        match key.as_symbol() {
            Some(symbol) => Ok(self
                .symbol_description(symbol)?
                .unwrap_or_else(|| JsString::from_static(""))),
            None => key
                .to_js_string()
                .ok_or(RuntimeError::Invariant("a string key")),
        }
    }

    pub fn new_symbol(&self, description: Option<JsString>) -> Result<SymbolRef, RuntimeError> {
        let mut entry = self.enter()?;
        let units: Option<Vec<u16>> = description.map(|text| text.utf16_units().collect());
        let symbol = entry.inner.core.new_symbol(units.as_deref())?;
        match entry.lift(symbol)? {
            Value::Symbol(symbol) => Ok(symbol),
            _ => Err(RuntimeError::Invariant("a new symbol")),
        }
    }

    pub fn symbol_description(&self, symbol: &SymbolRef) -> Result<Option<JsString>, RuntimeError> {
        let entry = self.enter()?;
        let value = entry.own_value(&symbol.slot)?;
        entry
            .core
            .symbol_description(value)
            .map(JsString::try_from_utf16)
            .transpose()
            .map_err(RuntimeError::from)
    }

    pub fn well_known_symbol(&self, symbol: WellKnownSymbol) -> Result<SymbolRef, RuntimeError> {
        let mut entry = self.enter()?;
        let value = entry
            .core
            .well_known_symbol(symbol.name())
            .ok_or(RuntimeError::Invariant("a well-known symbol"))?;
        match entry.lift(value)? {
            Value::Symbol(symbol) => Ok(symbol),
            _ => Err(RuntimeError::Invariant("a well-known symbol")),
        }
    }

    // ---- objects ----

    pub fn get_prototype_of(&self, object: &ObjectRef) -> Result<Option<ObjectRef>, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let result = entry.inner.core.get_prototype_of(id);
        let prototype = entry.complete_with(result)?;
        Ok(prototype.map(|prototype| entry.lift_object(prototype)))
    }

    pub fn set_prototype_of(
        &self,
        object: &ObjectRef,
        prototype: Option<&ObjectRef>,
    ) -> Result<bool, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let prototype = prototype.map(|p| entry.object(p)).transpose()?;
        let result = entry.inner.core.set_prototype_of(id, prototype);
        entry.complete_with(result)
    }

    /// The object as a callable reference; `None` when it is not callable.
    pub fn as_callable(&self, object: &ObjectRef) -> Result<Option<CallableRef>, RuntimeError> {
        let entry = self.enter()?;
        let value = entry.own_value(&object.slot)?;
        Ok(entry
            .core
            .is_callable(value)
            .then(|| CallableRef(object.clone())))
    }

    pub fn is_constructor(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        let entry = self.enter()?;
        let value = entry.own_value(&object.slot)?;
        Ok(entry.core.is_constructor(value))
    }

    /// `IsArray`.
    pub fn is_array_object(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        let mut entry = self.enter()?;
        let value = entry.own_value(&object.slot)?;
        let result = entry.inner.core.is_array(value);
        entry.complete_with(result)
    }

    pub fn is_error_object(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        let entry = self.enter()?;
        let value = entry.own_value(&object.slot)?;
        Ok(entry.core.is_error(value))
    }

    pub fn define_own_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
        descriptor: &OrdinaryPropertyDescriptor,
    ) -> Result<bool, RuntimeError> {
        self.context().define_own_property(object, key, descriptor)
    }

    pub fn get_own_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<Option<CompleteOrdinaryPropertyDescriptor>, RuntimeError> {
        self.context().get_own_property(object, key)
    }

    pub fn delete_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let key = entry.key(key)?;
        let result = entry.inner.core.delete_property(id, key);
        entry.complete_with(result)
    }

    pub fn has_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let key = entry.key(key)?;
        let result = entry.inner.core.has_property(id, key);
        entry.complete_with(result)
    }

    pub fn has_own_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let key = entry.key(key)?;
        let result = entry.inner.core.has_own_property(id, key);
        entry.complete_with(result)
    }

    pub fn own_property_keys(&self, object: &ObjectRef) -> Result<Vec<PropertyKey>, RuntimeError> {
        let mut entry = self.enter()?;
        let id = entry.object(object)?;
        let result = entry.inner.core.own_property_keys(id);
        let keys = entry.complete_with(result)?;
        keys.into_iter().map(|key| entry.lift_key(key)).collect()
    }

    /// `typeof value`.
    pub fn type_of(&self, value: &Value) -> Result<String, RuntimeError> {
        let mut entry = self.enter()?;
        let value = entry.lower(value)?;
        Ok(entry.inner.core.type_of(value))
    }

    /// QuickJS's `JS_PrintValue`: what `print` and an uncaught-exception dump
    /// show. Runs no JavaScript (no accessor, trap or coercion).
    pub fn print_value(&self, value: &Value) -> Result<String, RuntimeError> {
        let mut entry = self.enter()?;
        let value = entry.lower(value)?;
        Ok(entry.inner.core.print_value(value))
    }

    /// [`Runtime::print_value`] as bytes (the current API's form).
    pub fn qjs_print_value_bytes(&self, value: &Value) -> Result<Vec<u8>, RuntimeError> {
        self.print_value(value).map(String::into_bytes)
    }
}

impl Entry<'_> {
    pub(crate) fn own_value(&self, slot: &Slot) -> Result<oxide_core::Value, RuntimeError> {
        self.own(slot)
    }
}
