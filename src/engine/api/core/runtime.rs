//! The API's runtime: one `Rc<RefCell<…>>` at the
//! boundary around `oxide_core::Runtime`. Every method borrows the cell for
//! one engine entry; a host function or callback that calls back in through
//! a `Context` or `Runtime` instead of its scope gets
//! [`RuntimeError::Reentered`].

use std::cell::{Cell, RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use oxide_core::Handle;
use oxide_core::host::Host;
use oxide_core::vm::module::ModuleTables;
use oxide_core::{MemoryReport, NativeCx, RuntimeOptions};

use super::RuntimeError;
use super::context::Context;
use super::module::ModuleLoader;
use super::ops::View;
use super::property::{
    CompleteOrdinaryPropertyDescriptor, OrdinaryPropertyDescriptor, PropertyKey, WellKnownSymbol,
};
use super::value::{CallableRef, ObjectRef, Slot, SymbolRef, Value};
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::value::JsString;

/// Interrupts a runtime's running script from any thread: the script stops
/// at its next poll (about every thousand loop back-edges and calls)
/// with [`RuntimeError::Interrupted`], uncatchable by the script. The flag
/// stays set until [`InterruptHandle::clear`], so every script after it is
/// interrupted too.
#[derive(Clone, Debug)]
pub struct InterruptHandle {
    flag: Arc<AtomicBool>,
}

impl InterruptHandle {
    pub fn interrupt(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }

    pub fn clear(&self) {
        self.flag.store(false, Ordering::Relaxed);
    }

    pub fn is_set(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

/// A runtime: one heap, one realm, one job queue. Cloning shares it.
#[derive(Clone)]
pub struct Runtime {
    pub(crate) shared: Rc<Shared>,
}

/// A module compiled by `Context::compile_module*` or defined by
/// `Context::define_module`, waiting for the core to load it by name.
pub(crate) enum Preloaded {
    Compiled {
        function: ObjectRef,
        tables: ModuleTables,
        import_meta: Vec<(String, Value)>,
    },
    Synthetic(Vec<(String, Value)>),
}

pub(crate) struct Shared {
    pub(crate) inner: RefCell<Inner>,
    /// Handles whose last reference dropped while the runtime was borrowed;
    /// released by the next entry or when the host function running ends.
    released: RefCell<Vec<Handle>>,
    /// A host function panicked: every later entry fails.
    pub(crate) poisoned: Cell<bool>,
    /// The core's interrupt flag, which `InterruptHandle`s share.
    interrupt: Arc<AtomicBool>,
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

    /// Releases the handles dropped while the runtime was borrowed.
    pub(crate) fn drain_released(&self, core: &mut NativeCx<'_>) {
        let released = std::mem::take(&mut *self.released.borrow_mut());
        for handle in released {
            core.release(handle);
        }
    }
}

pub(crate) struct Inner {
    pub(crate) core: oxide_core::Runtime,
}

/// Host stack an outermost entry keeps below the loop's re-entry budget for
/// the frames that build and unwind a stack overflow error.
const NATIVE_RESERVE: usize = if cfg!(debug_assertions) {
    256 * 1024
} else {
    64 * 1024
};

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
    }
}

impl Entry<'_> {
    /// The operations' view of the entered runtime.
    pub(crate) fn view(&mut self) -> View<'_> {
        View::new(self.inner.core.native_cx(), self.shared)
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
        shared: Rc::downgrade(shared),
        handle,
        value,
    })
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
    pub(crate) fn new(state: PromiseState, result: Value) -> Self {
        PromiseSnapshot { state, result }
    }

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
        let interrupt = core.native_cx().interrupt_flag();
        Runtime {
            shared: Rc::new(Shared {
                interrupt,
                inner: RefCell::new(Inner { core }),
                released: RefCell::new(Vec::new()),
                poisoned: Cell::new(false),
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
        if self.shared.poisoned.get() {
            return Err(RuntimeError::Poisoned);
        }
        let mut inner = self
            .shared
            .inner
            .try_borrow_mut()
            .map_err(|_| RuntimeError::Reentered)?;
        self.shared.drain_released(&mut inner.core.native_cx());
        if CURRENT.with(|current| current.borrow().is_empty()) {
            inner.core.native_cx().clear_interrupted();
            // The loop's re-entries may use the thread's stack past what this
            // entry needs to unwind an error: QuickJS's one-MiB budget, which
            // its frames reach at the depths its nesting tests pin, takes
            // larger debug frames here.
            // An undeclared thread keeps the default as a ceiling.
            let (available, declared) = crate::engine::compiler::stack_guard::available_stack();
            let budget = available
                .saturating_sub(available / 8)
                .saturating_sub(NATIVE_RESERVE)
                .max(available / 2);
            let budget = if declared {
                budget
            } else {
                budget.min(oxide_core::vm::NATIVE_STACK_BUDGET)
            };
            inner.core.set_native_stack_budget(budget);
        }
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

    /// The handle another thread interrupts this runtime's scripts with.
    pub fn interrupt_handle(&self) -> InterruptHandle {
        InterruptHandle {
            flag: Arc::clone(&self.shared.interrupt),
        }
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

    /// Runs the oldest pending job. A job that throws returns
    /// `RuntimeError::Exception` with the thrown value; the queue keeps the
    /// jobs after it. Rejected inside an execution turn,
    /// leaving the job queued.
    pub fn run_pending_job(&self) -> Result<PendingJobOutcome, RuntimeError> {
        if self.shared.turns.get() > 0 {
            return Err(RuntimeError::Invariant(
                "a job was dispatched inside an execution turn",
            ));
        }
        let mut entry = self.enter()?;
        let result = entry.inner.core.run_pending_job();
        match entry.view().complete_with(result)? {
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
        self.enter()?.view().promise_snapshot(promise)
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
        let promises = entry.inner.core.unhandled_rejections().to_vec();
        let mut rejections = Vec::with_capacity(promises.len());
        let mut view = entry.view();
        for promise in promises {
            let reason = view
                .core
                .promise_state(promise)
                .map_or(oxide_core::Value::UNDEFINED, |(_, reason)| reason);
            let promise = view.lift(promise)?;
            let reason = view.lift(reason)?;
            if let Value::Object(promise) = promise {
                rejections.push((promise, reason));
            }
        }
        drop(view);
        entry.inner.core.clear_unhandled_rejections();
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
            .set_gc_manual(policy == GcPolicy::Manual);
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
        for unit in entry.core.units().tree(lowered.root) {
            text.push_str(&entry.core.disassemble(unit));
            text.push('\n');
        }
        Ok(text)
    }

    /// The new front end's syntax tree of `source` (`--dump-ast`).
    pub fn dump_ast(&self, source: &[u8], module: bool) -> Result<String, RuntimeError> {
        let program = parse_new_front_end(source, module)?;
        Ok(format!("{:#?}\n", program.root))
    }

    /// The new front end's HIR of `source` (`--dump-hir`).
    pub fn dump_hir(
        &self,
        source: &[u8],
        filename: &str,
        module: bool,
    ) -> Result<String, RuntimeError> {
        let program = parse_new_front_end(source, module)?;
        if program.has_with {
            return Err(RuntimeError::Internal(
                "SyntaxError: with statements are not supported".into(),
            ));
        }
        let built =
            crate::engine::hir::build::build(program, filename).map_err(RuntimeError::Compile)?;
        if let Err(error) = crate::engine::hir::validate::validate(&built.program) {
            return Err(RuntimeError::Internal(error.to_string()));
        }
        Ok(crate::engine::hir::schema::dump(&built.program))
    }

    /// The new front end's MIR of `source` (`--dump-mir`).
    pub fn dump_mir(
        &self,
        source: &[u8],
        filename: &str,
        module: bool,
    ) -> Result<String, RuntimeError> {
        let program = parse_new_front_end(source, module)?;
        let has_with = program.has_with;
        let built =
            crate::engine::hir::build::build(program, filename).map_err(RuntimeError::Compile)?;
        let mir = crate::engine::mir::build::build(&built.program, &built.regexps, has_with)
            .map_err(|error| RuntimeError::Internal(error.to_string()))?;
        let mut out = String::new();
        for function in &mir.functions {
            out.push_str(&crate::engine::mir::dump::dump(function));
            out.push('\n');
        }
        Ok(out)
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
        self.enter()?.view().new_symbol(description)
    }

    pub fn symbol_description(&self, symbol: &SymbolRef) -> Result<Option<JsString>, RuntimeError> {
        self.enter()?.view().symbol_description(symbol)
    }

    pub fn well_known_symbol(&self, symbol: WellKnownSymbol) -> Result<SymbolRef, RuntimeError> {
        self.enter()?.view().well_known_symbol(symbol)
    }

    // ---- objects ----

    pub fn get_prototype_of(&self, object: &ObjectRef) -> Result<Option<ObjectRef>, RuntimeError> {
        self.enter()?.view().get_prototype_of(object)
    }

    pub fn set_prototype_of(
        &self,
        object: &ObjectRef,
        prototype: Option<&ObjectRef>,
    ) -> Result<bool, RuntimeError> {
        self.enter()?.view().set_prototype_of(object, prototype)
    }

    /// The object as a callable reference; `None` when it is not callable.
    pub fn as_callable(&self, object: &ObjectRef) -> Result<Option<CallableRef>, RuntimeError> {
        self.enter()?.view().as_callable(object)
    }

    pub fn is_constructor(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        self.enter()?.view().is_constructor(object)
    }

    /// `IsArray`.
    pub fn is_array_object(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        self.enter()?.view().is_array_object(object)
    }

    pub fn is_error_object(&self, object: &ObjectRef) -> Result<bool, RuntimeError> {
        self.enter()?.view().is_error_object(object)
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
        self.enter()?.view().delete_property(object, key)
    }

    pub fn has_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        self.enter()?.view().has_property(object, key)
    }

    pub fn has_own_property(
        &self,
        object: &ObjectRef,
        key: &PropertyKey,
    ) -> Result<bool, RuntimeError> {
        self.enter()?.view().has_own_property(object, key)
    }

    pub fn own_property_keys(&self, object: &ObjectRef) -> Result<Vec<PropertyKey>, RuntimeError> {
        self.enter()?.view().own_property_keys(object)
    }

    /// `typeof value`.
    pub fn type_of(&self, value: &Value) -> Result<String, RuntimeError> {
        self.enter()?.view().type_of(value)
    }

    /// QuickJS's `JS_PrintValue`: what `print` and an uncaught-exception dump
    /// show. Runs no JavaScript (no accessor, trap or coercion).
    pub fn print_value(&self, value: &Value) -> Result<String, RuntimeError> {
        self.enter()?.view().print_value(value)
    }

    /// [`Runtime::print_value`] as the bytes QuickJS writes: a lone
    /// surrogate of a raw name, message or stack as its own three bytes
    /// (WTF-8) instead of U+FFFD.
    pub fn qjs_print_value_bytes(&self, value: &Value) -> Result<Vec<u8>, RuntimeError> {
        self.enter()?.view().qjs_print_value_bytes(value)
    }
}

fn parse_new_front_end(
    source: &[u8],
    module: bool,
) -> Result<crate::engine::ast::Program, RuntimeError> {
    let text = crate::source::text::SourceText::try_from_raw_bytes(source)
        .map_err(|error| RuntimeError::Compile(error.into()))?;
    let parsed = if module {
        crate::engine::ast::parser::Parser::parse_module(&text)
    } else {
        crate::engine::ast::parser::Parser::parse_script(&text)
    };
    parsed.map_err(RuntimeError::Compile)
}
