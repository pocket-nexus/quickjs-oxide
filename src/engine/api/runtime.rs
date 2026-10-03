//! Runtime creation, identity, and host configuration.

use crate::engine::atom::AtomTable;
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::heap::Heap;

use super::runtime_error::RuntimeError;
use crate::engine::heap::runtime::{
    NEXT_RUNTIME_DOMAIN_ID, RuntimeInner, RuntimeState, StateStorage,
};
use crate::engine::host::HostServices;
use crate::engine::object::WellKnownSymbol;

#[cfg(test)]
use quickjs_oxide_host::SystemHostServices;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::Ordering;

/// A borrowed unwind marker; it owns neither runtime state nor JS references.
/// Entry points which only retain/read state use it without draining cleanup.
pub(crate) struct RuntimeUnwindGuard<'a>(&'a Cell<bool>);

impl<'a> RuntimeUnwindGuard<'a> {
    pub(crate) fn from_flag(poisoned: &'a Cell<bool>) -> Self {
        Self(poisoned)
    }
}

impl Drop for RuntimeUnwindGuard<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.set(true);
        }
    }
}

impl Runtime {
    /// Whether a panic has quarantined this runtime. Identity queries and
    /// dropping handles remain available; state operations return `Poisoned`.
    #[must_use]
    pub fn is_poisoned(&self) -> bool {
        self.0.poisoned.get()
    }

    #[inline]
    pub(crate) fn check_poison(&self) -> Result<(), RuntimeError> {
        if self.skip_cleanup() {
            Err(RuntimeError::Poisoned)
        } else {
            Ok(())
        }
    }

    pub(crate) fn unwind_guard(&self) -> RuntimeUnwindGuard<'_> {
        RuntimeUnwindGuard::from_flag(&self.0.poisoned)
    }

    /// Cleanup during unwind must not traverse partially mutated state. Mark
    /// quarantine before any nested root destructor has a chance to run.
    #[inline]
    pub(crate) fn skip_cleanup(&self) -> bool {
        if std::thread::panicking() {
            self.0.poisoned.set(true);
            true
        } else {
            self.is_poisoned()
        }
    }

    #[must_use]
    #[cfg(test)]
    pub fn new() -> Self {
        Self::new_with_host_services(SystemHostServices::default())
    }

    /// Create a runtime with embedder-provided clock, time-zone, and random
    /// seed services.
    ///
    /// The services are called synchronously and retained for the runtime's
    /// lifetime. The application selects the concrete provider at this embedding boundary.
    #[must_use]
    pub fn new_with_host_services(host_services: impl HostServices + 'static) -> Self {
        Self::new_configured(
            host_services,
            #[cfg(feature = "profiling")]
            None,
        )
    }

    fn new_configured(
        host_services: impl HostServices + 'static,
        #[cfg(feature = "profiling")] trace: Option<super::profiling::AllocationTrace>,
    ) -> Self {
        let host_services: Rc<dyn HostServices> = Rc::new(host_services);
        let domain_id = NEXT_RUNTIME_DOMAIN_ID.fetch_add(1, Ordering::Relaxed);
        assert_ne!(domain_id, 0, "runtime domain ID space exhausted");
        let mut atoms = AtomTable::with_static_atoms(crate::engine::vm::TYPEOF_STATIC_ATOMS)
            .expect("fixed typeof atom set fits the atom table");
        let pinned_atoms = crate::engine::atom::pinned::PinnedAtoms::new(&mut atoms)
            .expect("static property atoms fit");
        let mut well_known_symbols = HashMap::new();
        for symbol in WellKnownSymbol::ALL {
            let atom = atoms
                .new_static_symbol(Some(symbol.description()))
                .expect("fixed well-known symbol set fits the atom table");
            well_known_symbols.insert(symbol, atom);
        }
        let active_frame_depth = Rc::new(Cell::new(0));
        let gc_pressure = Rc::new(crate::engine::heap::gc_pressure::GcPressure::new());
        Self(Rc::new(RuntimeInner {
            poisoned: Cell::new(false),
            raw_execution_owners: Cell::new(0),
            execution_turn_depth: Cell::new(0),
            gc_pressure: gc_pressure.clone(),
            state: StateStorage::new(RuntimeState {
                kept_objects: Default::default(),
                atoms,
                pinned_atoms,
                heap: {
                    #[cfg(feature = "profiling")]
                    {
                        Heap::with_allocation_trace(trace)
                    }
                    #[cfg(not(feature = "profiling"))]
                    Heap::new()
                }
                .with_gc_pressure(gc_pressure),
                pending_exception: None,
                pending_jobs: VecDeque::new(),
                debug_info_mode: DebugInfoMode::Full,
                retained_shapes: Default::default(),
                shape_cache: HashMap::default(),
                shape_hashes: HashMap::default(),
                shape_transitions: HashMap::default(),
                shape_transition_parents: HashMap::default(),
                well_known_symbols,
                proxy_trap_reads: std::array::from_fn(|_| {
                    crate::engine::object::property_ic::PropertyReadCache::default()
                }),
                active_frames: crate::engine::vm::frames::ActiveFrames::with_depth(
                    active_frame_depth.clone(),
                ),
                active_collection_records: Vec::new(),
                next_active_frame_token: 1,
                next_module_async_evaluation_order: 0,
                #[cfg(test)]
                active_frame_probe_snapshots: Vec::new(),
                #[cfg(test)]
                iterator_result_allocations: 0,
            }),
            active_frame_depth,
            deferred_references: Default::default(),
            host_services,
            can_block: Cell::new(false),
            promise_rejection_tracker: RefCell::new(None),
            module_loader: RefCell::new(None),
            #[cfg(feature = "test262-host")]
            dynamic_import_bytecode_allowed: Cell::new(true),
            module_host_callback_depth: Cell::new(0),
            host_stack_top: Cell::new(None),
            proxy_method_depth: Cell::new(0),
            recursion_limit: Cell::new(u16::MAX as usize),
            next_context_id: Cell::new(0),
            domain_id,
        }))
    }

    /// Start a bounded, partial allocation trace before runtime initialization.
    /// The returned handle owns diagnostic records, never runtime/JS roots.
    /// Read it after dropping every context, value and runtime handle to include
    /// teardown. See `AllocationTrace` for the exact coverage contract.
    #[cfg(feature = "profiling")]
    #[must_use]
    pub fn new_with_allocation_trace(
        host_services: impl HostServices + 'static,
        max_events: usize,
    ) -> (Self, super::profiling::AllocationTrace) {
        let trace = super::profiling::AllocationTrace::new(max_events);
        let runtime = Self::new_configured(host_services, Some(trace.clone()));
        trace.set_runtime_id(runtime.domain_id());
        (runtime, trace)
    }

    /// Set the runtime-wide debug information policy for future compilations.
    /// Existing bytecode is immutable and keeps the mode used when published.
    pub fn set_debug_info_mode(&self, mode: DebugInfoMode) -> Result<(), RuntimeError> {
        self.check_poison()?;
        let _operation = self.operation();
        self.0.state.borrow_mut().debug_info_mode = mode;
        Ok(())
    }

    /// Return the policy which the next compilation will sample.
    #[must_use]
    pub fn debug_info_mode(&self) -> Result<DebugInfoMode, RuntimeError> {
        self.check_poison()?;
        let _unwind = self.unwind_guard();
        Ok(self.0.state.borrow().debug_info_mode)
    }

    /// Set whether this runtime's host permits synchronous blocking operations.
    ///
    /// The setting is runtime-wide, so cloned handles and every context owned
    /// by this runtime observe the same value. As in QuickJS, new runtimes
    /// default to `false` and embedders must opt in explicitly.
    pub fn set_can_block(&self, can_block: bool) -> Result<(), RuntimeError> {
        self.check_poison()?;
        self.0.can_block.set(can_block);
        Ok(())
    }

    /// Return whether this runtime's host permits synchronous blocking.
    #[must_use]
    pub fn can_block(&self) -> Result<bool, RuntimeError> {
        self.check_poison()?;
        Ok(self.0.can_block.get())
    }

    #[must_use]
    pub fn is_same_runtime(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }

    /// Stable identity used by rooted handle hashing and diagnostics.
    #[must_use]
    pub fn domain_id(&self) -> u64 {
        self.0.domain_id
    }

    /// Set the maximum number of installed JavaScript call frames for one
    /// top-level execution. This is the JavaScript-frame ceiling only; the
    /// native host-stack budget that protects Rust reentry is independent and
    /// unaffected.
    ///
    /// The value is sampled when a top-level execution starts, so already
    /// running executions keep the limit they began with. A limit of `0` is
    /// raised to `1`.
    pub fn set_recursion_limit(&self, limit: usize) -> Result<(), RuntimeError> {
        self.check_poison()?;
        self.0.recursion_limit.set(limit.max(1));
        Ok(())
    }

    /// Return the configured JavaScript call-frame recursion limit.
    #[must_use]
    pub fn recursion_limit(&self) -> Result<usize, RuntimeError> {
        self.check_poison()?;
        Ok(self.0.recursion_limit.get())
    }
}

/// A single-threaded QuickJS-compatible runtime.
///
/// Cloning this handle does not clone the runtime; it creates another owner of
/// the same heap/atom domain so multiple contexts can share runtime resources.
pub struct Runtime(pub(crate) Rc<RuntimeInner>);

impl Clone for Runtime {
    #[inline]
    fn clone(&self) -> Self {
        #[cfg(feature = "profiling")]
        super::profiling::record_runtime_event("runtime.clone", "core.runtime_clone");
        Self(Rc::clone(&self.0))
    }
}

#[cfg(test)]
impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod recursion_limit_tests {
    use super::*;
    use crate::engine::value::{JsString, Value};

    // Deep enough to exceed a small configured limit but not the default one.
    const DEEP: &str = "(function(){try{(function f(n){return n<=0?0:1+f(n-1)})(5000);return 'ok'}catch(e){return e.message}})()";

    #[test]
    fn runtime_recursion_limit_is_configurable_and_default_is_unchanged() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            runtime.recursion_limit().expect("runtime configuration"),
            u16::MAX as usize
        );
        assert_eq!(
            context.eval(DEEP).unwrap(),
            Value::String(JsString::from_static("ok"))
        );

        runtime
            .set_recursion_limit(200)
            .expect("set runtime configuration");
        assert_eq!(
            runtime.recursion_limit().expect("runtime configuration"),
            200
        );
        let Value::String(message) = context.eval(DEEP).unwrap() else {
            panic!("expected a message string")
        };
        assert!(message.to_string().contains("stack overflow"), "{message}");

        // A zero limit is clamped to one; a very low but usable limit still
        // produces a catchable overflow from inside JavaScript.
        runtime
            .set_recursion_limit(0)
            .expect("set runtime configuration");
        assert_eq!(runtime.recursion_limit().expect("runtime configuration"), 1);
        runtime
            .set_recursion_limit(10)
            .expect("set runtime configuration");
        let Value::String(message) = context.eval(DEEP).unwrap() else {
            panic!("expected a message string")
        };
        assert!(message.to_string().contains("stack overflow"), "{message}");
    }
}

#[cfg(all(test, not(target_family = "wasm"), panic = "unwind"))]
mod poison_tests {
    use super::*;
    use crate::engine::api::error::ErrorKind;
    use crate::engine::object::{PropertyKey, WellKnownSymbol};
    use crate::engine::value::Value;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    // An abort caused by a secondary destructor panic must fail only the
    // subprocess, not terminate the test suite before it can report the bug.
    fn subprocess(case: &str) {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "engine::api::runtime::poison_tests::poison_child",
                "--nocapture",
            ])
            .env("QJS_POISON_CHILD", case)
            .status()
            .unwrap();
        assert!(status.success(), "poison subprocess failed: {status}");
    }

    #[test]
    fn partial_mutation_quarantines_roots_and_runtime_teardown() {
        subprocess("partial-mutation");
    }

    #[test]
    fn host_caught_child_panic_prevents_parent_resume() {
        subprocess("host-caught-panic");
    }

    #[test]
    fn module_host_catching_child_panic_cannot_resume_resolution() {
        subprocess("module-host-caught-panic");
    }

    #[cfg(debug_assertions)]
    #[test]
    fn panic_during_operation_entry_drain_quarantines_runtime() {
        subprocess("entry-drain-panic");
    }

    #[derive(Debug)]
    struct PanicClock;
    impl HostServices for PanicClock {
        fn now_millis(&self) -> i64 {
            panic!("injected clock panic")
        }
        fn timezone_offset_minutes(&self, _: i64) -> i32 {
            0
        }
        fn random_seed(&self) -> u64 {
            1
        }
    }

    #[derive(Debug)]
    struct CatchingLoader;
    impl crate::engine::modules::ModuleLoader for CatchingLoader {
        fn load(
            &self,
            context: &mut crate::engine::api::Context,
            _: &crate::engine::value::JsString,
            _: &crate::engine::code::module::ModuleImportAttributes,
        ) -> Result<
            crate::engine::modules::ModuleLoadResult,
            crate::engine::modules::ModuleLoaderError,
        > {
            let nested = catch_unwind(AssertUnwindSafe(|| context.eval("Date.now()")));
            assert!(nested.is_err());
            Ok(crate::engine::modules::ModuleLoadResult::SourceText(
                "export const value = 1;".to_owned(),
            ))
        }
    }

    #[test]
    fn normal_errors_preserve_cleanup_and_runtime_reuse() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        assert_eq!(
            context.eval("throw new Error('normal throw')").unwrap_err(),
            RuntimeError::Exception
        );
        assert!(context.take_exception().unwrap().is_some());
        assert_eq!(context.eval("40 + 2").unwrap(), Value::Int(42));
        assert!(!runtime.is_poisoned());
        let before = runtime.heap_counts().unwrap().object_nodes;
        let owner = runtime.new_object(None).unwrap();
        assert_eq!(runtime.heap_counts().unwrap().object_nodes, before + 1);
        drop(owner);
        assert_eq!(runtime.heap_counts().unwrap().object_nodes, before);
        drop(context);
        runtime.run_gc().unwrap();
    }

    #[test]
    fn poison_child() {
        let Ok(case) = std::env::var("QJS_POISON_CHILD") else {
            return;
        };
        let runtime = if case == "module-host-caught-panic" {
            Runtime::new_with_host_services(PanicClock)
        } else {
            Runtime::new()
        };
        let mut context = runtime.new_context().unwrap();
        let object = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("owned-key").unwrap();
        let symbol = runtime.new_symbol(None).unwrap();
        let bytecode = context.compile("1").unwrap();
        match case.as_str() {
            #[cfg(debug_assertions)]
            "entry-drain-panic" => {
                let id = object.object_id();
                runtime
                    .0
                    .state
                    .borrow_mut()
                    .heap
                    .release_object(id)
                    .unwrap();
                runtime
                    .0
                    .deferred_references
                    .push_back(crate::engine::heap::runtime::DeferredRefOp::Object(id));
                let failed = catch_unwind(AssertUnwindSafe(|| {
                    // The drain fails before RuntimeOperation is constructed.
                    let _operation = runtime.operation();
                }));
                assert!(failed.is_err());
            }
            "partial-mutation" => {
                let failed = catch_unwind(AssertUnwindSafe(|| {
                    let _operation = runtime.operation();
                    let mut state = runtime.0.state.borrow_mut();
                    // Simulate interrupted ownership accounting: the public
                    // root is deliberately stale after this mutation. No
                    // destructor may try to release or validate it on unwind.
                    state.heap.release_object(object.object_id()).unwrap();
                    panic!("injected panic after destructive mutation");
                }));
                assert!(failed.is_err());
            }
            "host-caught-panic" => {
                let boundary = crate::engine::vm::HostBoundaryGuard::enter(&runtime).unwrap();
                let caught = catch_unwind(AssertUnwindSafe(|| {
                    let _execution = runtime.enter_execution_turn().unwrap();
                    let _operation = runtime.operation();
                    panic!("host catches nested engine panic");
                }));
                assert!(caught.is_err());
                let error = boundary.finish(&runtime).unwrap_err();
                assert_eq!(error.kind(), ErrorKind::Internal);
            }
            "module-host-caught-panic" => {
                let _loader = runtime.set_module_loader(CatchingLoader).unwrap();
                let error = context
                    .compile_module_with_filename("import './child.js';", "parent.js")
                    .unwrap_err();
                assert_eq!(error, RuntimeError::Poisoned);
            }
            _ => panic!("unknown poison subprocess"),
        }
        assert!(runtime.is_poisoned());
        assert!(matches!(runtime.new_context(), Err(RuntimeError::Poisoned)));
        assert_eq!(runtime.heap_counts(), Err(RuntimeError::Poisoned));
        assert_eq!(runtime.is_job_pending(), Err(RuntimeError::Poisoned));
        assert!(matches!(runtime.run_gc(), Err(RuntimeError::Poisoned)));
        assert!(matches!(object.try_clone(), Err(RuntimeError::Poisoned)));
        assert!(matches!(key.try_clone(), Err(RuntimeError::Poisoned)));
        assert!(matches!(symbol.try_clone(), Err(RuntimeError::Poisoned)));
        assert!(matches!(bytecode.try_clone(), Err(RuntimeError::Poisoned)));
        assert!(matches!(context.try_clone(), Err(RuntimeError::Poisoned)));
        assert!(matches!(context.eval("1"), Err(RuntimeError::Poisoned)));
        assert!(matches!(
            runtime.well_known_symbol(WellKnownSymbol::Iterator),
            Err(RuntimeError::Poisoned)
        ));
        assert!(matches!(
            PropertyKey::try_from(&symbol),
            Err(RuntimeError::Poisoned)
        ));
        let rooted = Value::Object(object);
        assert!(matches!(rooted.to_boolean(), Err(RuntimeError::Poisoned)));
        drop((rooted, key, symbol, bytecode, context));
        drop(runtime);
    }
}
