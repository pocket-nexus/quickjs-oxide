//! Scoped execution ownership. The registry contains identities, never Values
//! or Runtime-owning frames, and its guard unregisters during Rust unwinding.

use crate::engine::api::error::Error;
use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::value::JsValue;
use crate::engine::vm::frame::FrameStore;
use crate::engine::vm::stack::SlotStore;
use std::cell::{Cell, RefCell};

thread_local! {
    static NEXT_EXECUTION: Cell<u64> = const { Cell::new(1) };
    static ACTIVE_EXECUTIONS: RefCell<Vec<ExecutionRegistration>> = const { RefCell::new(Vec::new()) };
    static HOST_BOUNDARIES: RefCell<Vec<HostBoundary>> = const { RefCell::new(Vec::new()) };
}

pub(super) struct ExecutionLimits {
    pub frames: usize,
    pub slots: usize,
}

impl Default for ExecutionLimits {
    fn default() -> Self {
        // Native recursion still uses its existing budget during migration.
        // Arena sizes are fallible and bounded by Rust's addressable storage;
        // S04 adds the explicit-call budget at the frame push boundary.
        Self {
            frames: u16::MAX as usize,
            slots: isize::MAX as usize
                / std::mem::size_of::<Option<crate::engine::vm::bindings::FrameBinding>>(),
        }
    }
}

impl ExecutionLimits {
    /// JavaScript-frame ceiling sampled from the runtime configuration. The
    /// slot budget keeps its default; the native host-stack budget is separate.
    pub(super) fn for_runtime(runtime: &Runtime) -> Self {
        Self {
            frames: runtime.0.recursion_limit.get(),
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ExecutionRegistration {
    domain: u64,
    id: u64,
    host_boundary: Option<u64>,
}

#[derive(Clone, Copy)]
struct HostBoundary {
    domain: u64,
    id: u64,
    parent_execution: Option<u64>,
    parent_frame: Option<super::frames::ActiveFrameToken>,
}

fn next_identity() -> Result<u64, Error> {
    NEXT_EXECUTION.with(|next| {
        let id = next.get();
        next.set(
            id.checked_add(1)
                .ok_or_else(|| Error::internal("execution identity exhausted"))?,
        );
        Ok(id)
    })
}

fn active_execution(domain: u64) -> Option<u64> {
    ACTIVE_EXECUTIONS.with(|active| {
        active
            .borrow()
            .iter()
            .rev()
            .find(|entry| entry.domain == domain)
            .map(|entry| entry.id)
    })
}

/// A real synchronous host callback may create another entry, but cannot
/// transfer ownership of its suspended parent's frames. This guard records
/// identities only; all registry and heap borrows end before calling host code.
/// Internal native calls do not create these delimiters.
#[must_use]
pub(crate) struct HostBoundaryGuard {
    boundary: HostBoundary,
    #[cfg(feature = "profiling")]
    _origin: crate::engine::api::profiling::CoreExecutionScope,
    runtime: std::rc::Weak<crate::engine::heap::runtime::RuntimeInner>,
}

impl HostBoundaryGuard {
    pub(crate) fn enter(runtime: &Runtime) -> Result<Self, Error> {
        runtime
            .check_poison()
            .map_err(super::exception::runtime_error_to_vm_error)?;
        let domain = runtime.domain_id();
        let boundary = HostBoundary {
            domain,
            id: next_identity()?,
            parent_execution: active_execution(domain),
            parent_frame: runtime
                .0
                .state
                .borrow()
                .active_frames
                .last()
                .map(|frame| frame.token),
        };
        HOST_BOUNDARIES.with(|boundaries| -> Result<(), Error> {
            let mut boundaries = boundaries.borrow_mut();
            boundaries
                .try_reserve(1)
                .map_err(|_| Error::internal("host boundary registration allocation failed"))?;
            boundaries.push(boundary);
            Ok(())
        })?;
        Ok(Self {
            boundary,
            runtime: std::rc::Rc::downgrade(&runtime.0),
            #[cfg(feature = "profiling")]
            _origin: crate::engine::api::profiling::CoreExecutionScope::outside(),
        })
    }

    pub(crate) fn finish(self, runtime: &Runtime) -> Result<(), Error> {
        // A host may have caught an inner panic. Never resume its suspended
        // parent against state quarantined by the child.
        runtime
            .check_poison()
            .map_err(super::exception::runtime_error_to_vm_error)?;
        let boundary = self.boundary;
        if runtime.domain_id() != boundary.domain {
            return Err(Error::internal("host boundary belongs to another runtime"));
        }
        let is_current = HOST_BOUNDARIES.with(|boundaries| {
            boundaries.borrow().last().is_some_and(|current| {
                current.id == boundary.id && current.domain == boundary.domain
            })
        });
        if !is_current {
            return Err(Error::internal("host boundaries returned out of order"));
        }
        let has_child = ACTIVE_EXECUTIONS.with(|active| {
            active.borrow().iter().any(|entry| {
                entry.domain == boundary.domain && entry.host_boundary == Some(boundary.id)
            })
        });
        if has_child || active_execution(boundary.domain) != boundary.parent_execution {
            return Err(Error::internal(
                "host callback left an execution registered",
            ));
        }
        let parent_frame = runtime
            .0
            .state
            .borrow()
            .active_frames
            .last()
            .map(|frame| frame.token);
        if parent_frame != boundary.parent_frame {
            return Err(Error::internal(
                "host callback did not restore its parent frame",
            ));
        }
        Ok(())
    }
}

impl Drop for HostBoundaryGuard {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.upgrade().map(Runtime) {
            runtime.skip_cleanup();
        }
        HOST_BOUNDARIES.with(|boundaries| {
            let mut boundaries = boundaries.borrow_mut();
            if let Some(index) = boundaries.iter().rposition(|boundary| {
                boundary.domain == self.boundary.domain && boundary.id == self.boundary.id
            }) {
                boundaries.remove(index);
            }
        });
    }
}

struct ExecutionGuard {
    _turn: crate::engine::heap::runtime::execution_turn::ExecutionTurn,
    registration: ExecutionRegistration,
}

impl ExecutionGuard {
    fn enter(runtime: &Runtime) -> Result<Self, Error> {
        let id = next_identity()?;
        let domain = runtime.domain_id();
        let parent = active_execution(domain);
        let host_boundary = HOST_BOUNDARIES.with(|boundaries| {
            boundaries
                .borrow()
                .iter()
                .rev()
                .find(|boundary| boundary.domain == domain)
                .filter(|boundary| boundary.parent_execution == parent)
                .map(|boundary| boundary.id)
        });
        if parent.is_some() && host_boundary.is_none() {
            return Err(Error::internal(
                "internal callback attempted a nested root execution",
            ));
        }
        let turn = runtime
            .enter_execution_turn()
            .map_err(super::exception::runtime_error_to_vm_error)?;
        let registration = ExecutionRegistration {
            domain,
            id,
            host_boundary,
        };
        ACTIVE_EXECUTIONS.with(|active| -> Result<(), Error> {
            let mut active = active.borrow_mut();
            active
                .try_reserve(1)
                .map_err(|_| Error::internal("execution registration allocation failed"))?;
            active.push(registration);
            Ok(())
        })?;
        Ok(Self {
            registration,
            _turn: turn,
        })
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        ACTIVE_EXECUTIONS.with(|active| {
            let mut active = active.borrow_mut();
            if let Some(index) = active.iter().rposition(|entry| *entry == self.registration) {
                active.remove(index);
            }
        });
    }
}

pub(super) struct RunningExecution {
    pub frames: FrameStore,
    pub slots: SlotStore,
    pub query_storage: super::proxy_get_driver::QueryStorage,
    pub call_storage: super::frame::CallStorage,
    /// Cold completion owns its payload before the active window is cleared.
    pub pending: Option<JsValue>,
    /// Forwarded return/throw remains reachable throughout fallible retirement.
    pub pending_completion: Option<super::Completion>,
    /// Retained GetField2 result's classification, consumed by the immediate Call.
    pub selected_native: Option<crate::engine::object::LinkedNativeSelection>,
    /// A selected static read crossing into the existing getter/query driver.
    pub selected_named_read: Option<super::property_driver::SelectedNamedRead>,
    /// A typed root terminal result; never represented by a manufactured JS Value.
    pub root_descriptor: Option<super::entry::DescriptorReply>,
    pub root_query: Option<Box<super::proxy_get_driver::PendingProxyGet>>,
    // The execution never keeps its runtime alive; teardown releases through
    // the upgrade only when the runtime still exists.
    runtime: std::rc::Weak<crate::engine::heap::runtime::RuntimeInner>,
    _guard: ExecutionGuard,
}

impl Drop for RunningExecution {
    fn drop(&mut self) {
        let Some(runtime) = self.runtime.upgrade().map(Runtime) else {
            // The runtime (and its whole heap) died first; no edge release can
            // observe anything. Discard the storage without accounting.
            self.pending = None;
            self.pending_completion = None;
            self.selected_named_read = None;
            self.slots = SlotStore::new(0);
            return;
        };
        runtime.unregister_raw_execution_owner();
        if runtime.skip_cleanup() {
            self.pending = None;
            self.pending_completion = None;
            self.selected_named_read = None;
            self.slots = SlotStore::new(0);
            return;
        }
        let _unwind = runtime.unwind_guard();
        if let Some(pending) = self.pending.take() {
            if runtime.release_jsvalue(pending).is_err() || runtime.is_poisoned() {
                runtime.0.poisoned.set(true);
                return;
            }
        }
        if let Some(super::Completion::Return(value) | super::Completion::Throw(value)) =
            self.pending_completion.take()
        {
            if runtime.release_jsvalue(value).is_err() || runtime.is_poisoned() {
                runtime.0.poisoned.set(true);
                return;
            }
        }
        if let Some(selected) = self.selected_named_read.take() {
            selected.release(&runtime);
            if runtime.is_poisoned() {
                return;
            }
        }
        while let Some(mut frame) = self.frames.pop_current() {
            // Clear this child's captures and operands while its activation
            // and every enclosing native query still own their roots.
            if self
                .slots
                .clear_frame(&runtime, frame.window.take())
                .is_err()
            {
                // Window and edge validation failures invalidate further
                // cleanup; keep the poisoned runtime quarantined.
                runtime.0.poisoned.set(true);
                return;
            }
            if self
                .call_storage
                .recycle_legacy(&runtime, frame.cold)
                .is_err()
                || runtime.is_poisoned()
            {
                runtime.0.poisoned.set(true);
                return;
            }
        }
        if let Some(pending) = self.root_query.take() {
            pending.release(&runtime);
        }
    }
}

impl Runtime {
    /// One weak registration per execution record, never per frame or value.
    /// The header count lets teardown distinguish legitimate detached edges
    /// from leaks when the runtime dies before its execution record.
    pub(in crate::engine::vm) fn register_raw_execution_owner(
        &self,
    ) -> Result<std::rc::Weak<crate::engine::heap::runtime::RuntimeInner>, RuntimeError> {
        self.check_poison()?;
        let count =
            self.0
                .raw_execution_owners
                .get()
                .checked_add(1)
                .ok_or(RuntimeError::Invariant(
                    "raw execution owner count exhausted",
                ))?;
        self.0.raw_execution_owners.set(count);
        Ok(std::rc::Rc::downgrade(&self.0))
    }

    pub(in crate::engine::vm) fn unregister_raw_execution_owner(&self) {
        if let Some(count) = self.0.raw_execution_owners.get().checked_sub(1) {
            self.0.raw_execution_owners.set(count);
        } else {
            // Teardown cannot report an invariant failure or unwind again.
            self.0.poisoned.set(true);
        }
    }
}

impl RunningExecution {
    pub(super) fn new(runtime: &Runtime, limits: ExecutionLimits) -> Result<Self, Error> {
        let guard = ExecutionGuard::enter(runtime)?;
        let weak = runtime
            .register_raw_execution_owner()
            .map_err(super::exception::runtime_error_to_vm_error)?;
        Ok(Self {
            frames: FrameStore::new(guard.registration.id, limits.frames),
            slots: SlotStore::new(limits.slots),
            query_storage: super::proxy_get_driver::QueryStorage::default(),
            call_storage: super::frame::CallStorage::default(),
            pending: None,
            pending_completion: None,
            selected_native: None,
            selected_named_read: None,
            root_query: None,
            root_descriptor: None,
            runtime: weak,
            _guard: guard,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ACTIVE_EXECUTIONS, ExecutionLimits, HOST_BOUNDARIES, HostBoundaryGuard, RunningExecution,
    };
    use crate::engine::api::Runtime;
    use std::rc::Rc;

    #[test]
    fn nested_execution_registration_is_removed_on_panic() {
        let runtime = Runtime::new();
        let outer = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let before = ACTIVE_EXECUTIONS.with(|active| active.borrow().clone());
        assert_eq!(before.len(), 1);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _boundary = HostBoundaryGuard::enter(&runtime).unwrap();
            let _inner = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            assert_eq!(ACTIVE_EXECUTIONS.with(|active| active.borrow().len()), 2);
            panic!("exercise execution guard unwinding");
        }));
        assert!(result.is_err());
        assert_eq!(
            ACTIVE_EXECUTIONS.with(|active| active.borrow().clone()),
            before
        );
        drop(outer);
        assert!(ACTIVE_EXECUTIONS.with(|active| active.borrow().is_empty()));
    }

    #[test]
    fn identity_registration_cannot_keep_a_runtime_alive() {
        let runtime = Runtime::new();
        let weak = Rc::downgrade(&runtime.0);
        let execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let boundary = HostBoundaryGuard::enter(&runtime).unwrap();
        drop(runtime);
        assert!(weak.upgrade().is_none());
        drop(execution);
        drop(boundary);
        assert!(HOST_BOUNDARIES.with(|boundaries| boundaries.borrow().is_empty()));
        assert!(ACTIVE_EXECUTIONS.with(|active| active.borrow().is_empty()));
    }
    #[test]
    fn host_boundary_uses_parent_entry_and_runtime_identity() {
        let runtime = Runtime::new();
        let other_runtime = Runtime::new();
        let outer = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let boundary = HostBoundaryGuard::enter(&runtime).unwrap();
        let inner = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        assert_eq!(
            inner._guard.registration.host_boundary,
            Some(boundary.boundary.id)
        );
        let internal = RunningExecution::new(&runtime, ExecutionLimits::default());
        assert!(
            matches!(internal, Err(error) if error.message() == "internal callback attempted a nested root execution")
        );
        let foreign = RunningExecution::new(&other_runtime, ExecutionLimits::default()).unwrap();
        assert_eq!(foreign._guard.registration.host_boundary, None);
        drop((foreign, inner));
        boundary.finish(&runtime).unwrap();
        assert!(HOST_BOUNDARIES.with(|boundaries| boundaries.borrow().is_empty()));
        drop(outer);
    }

    #[test]
    fn host_boundary_restores_registry_after_callback_panic() {
        let runtime = Runtime::new();
        let outer = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let before = ACTIVE_EXECUTIONS.with(|active| active.borrow().clone());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _boundary = HostBoundaryGuard::enter(&runtime).unwrap();
            let _inner = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
            panic!("exercise host callback unwinding");
        }));
        assert!(result.is_err());
        assert_eq!(
            ACTIVE_EXECUTIONS.with(|active| active.borrow().clone()),
            before
        );
        assert!(HOST_BOUNDARIES.with(|boundaries| boundaries.borrow().is_empty()));
        drop(outer);
    }

    #[test]
    fn host_boundary_rejects_a_live_reentry_on_return() {
        let runtime = Runtime::new();
        let boundary = HostBoundaryGuard::enter(&runtime).unwrap();
        let inner = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        assert_eq!(
            boundary.finish(&runtime).unwrap_err().message(),
            "host callback left an execution registered"
        );
        assert!(HOST_BOUNDARIES.with(|boundaries| boundaries.borrow().is_empty()));
        drop(inner);
        assert!(ACTIVE_EXECUTIONS.with(|active| active.borrow().is_empty()));
    }

    #[test]
    fn module_host_reentry_preserves_owned_parent_bindings_and_unwinds() {
        use crate::engine::api::{
            Context, JsString, ModuleImportAttributes, ModuleLoadResult, ModuleLoader,
            ModuleLoaderError, Value,
        };
        use std::cell::{Cell, RefCell};

        #[derive(Clone, Copy, Debug)]
        enum Outcome {
            Return,
            Reject,
            Panic,
        }
        #[derive(Debug)]
        struct Loader {
            outcome: Outcome,
            calls: Rc<Cell<usize>>,
        }
        impl ModuleLoader for Loader {
            fn load(
                &self,
                context: &mut Context,
                _name: &JsString,
                _attributes: &ModuleImportAttributes,
            ) -> Result<ModuleLoadResult, ModuleLoaderError> {
                self.calls.set(self.calls.get() + 1);
                let before = ACTIVE_EXECUTIONS.with(|active| active.borrow().clone());
                let boundary =
                    HOST_BOUNDARIES.with(|boundaries| *boundaries.borrow().last().unwrap());
                // Host-triggered compilation runs while the bytecode caller
                // retains its captured bindings in the owned slot arena.
                assert_eq!(before.len(), 1);
                assert_eq!(boundary.parent_execution, Some(before[0].id));
                assert!(context.runtime().0.state.borrow().active_frames.len() >= 2);
                assert_eq!(context.eval("reenter()").unwrap(), Value::Int(41));
                context.runtime().run_gc().unwrap();
                assert_eq!(
                    ACTIVE_EXECUTIONS.with(|active| active.borrow().clone()),
                    before
                );
                match self.outcome {
                    Outcome::Return => Ok(ModuleLoadResult::SourceText("export {};".to_owned())),
                    Outcome::Reject => Err(ModuleLoaderError::exception(Value::Int(99))),
                    Outcome::Panic => panic!("exercise a real module-host callback panic"),
                }
            }
        }

        for outcome in [Outcome::Return, Outcome::Reject, Outcome::Panic] {
            let runtime = Runtime::new();
            let calls = Rc::new(Cell::new(0));
            let _registration = runtime.set_module_loader(Loader {
                outcome,
                calls: calls.clone(),
            });
            let mut context = runtime.new_context().expect("create context");
            let trigger = context.eval("Promise.reject.bind(Promise)").unwrap();
            let Value::Object(function) = context
                .eval(
                    r#"
                var reenter;
                (function(trigger){
                    let captured=1;
                    reenter=function(){captured=41;return captured};
                    var pending=trigger(1);
                    return captured+1;
                })
            "#,
                )
                .unwrap()
            else {
                panic!("expected function")
            };
            let callable = runtime.as_callable(&function).unwrap().unwrap();
            // The existing rejection-tracker ABI is the synchronous host
            // trigger. Module compilation then enters the guarded loader;
            // dynamic import itself would defer loading to a later job.
            let host_context = RefCell::new(context.try_clone().expect("duplicate root"));
            struct ClearTracker(Runtime);
            impl Drop for ClearTracker {
                fn drop(&mut self) {
                    let _ = self.0.clear_host_promise_rejection_tracker();
                }
            }
            let tracker_guard = ClearTracker(runtime.clone());
            runtime
                .set_host_promise_rejection_tracker(move |event| {
                    if event.is_handled() {
                        return;
                    }
                    let mut context = host_context.borrow_mut();
                    let result = context
                        .compile_module_with_filename("import './owned-host.js';", "host-entry.js");
                    match outcome {
                        Outcome::Return => {
                            result.unwrap();
                        }
                        Outcome::Reject => {
                            assert!(result.is_err());
                            assert_eq!(context.take_exception().unwrap(), Some(Value::Int(99)));
                        }
                        Outcome::Panic => unreachable!("loader should have panicked"),
                    }
                })
                .expect("configure test runtime");
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                context.call(&callable, Value::Undefined, &[trigger])
            }));
            drop(tracker_guard);
            match outcome {
                Outcome::Panic => {
                    let payload = result.expect_err("expected host callback panic");
                    assert_eq!(
                        payload.downcast_ref::<&str>().copied(),
                        Some("exercise a real module-host callback panic")
                    );
                }
                _ => assert_eq!(result.unwrap().unwrap(), Value::Int(42)),
            }
            assert_eq!(calls.get(), 1);
            assert!(ACTIVE_EXECUTIONS.with(|active| active.borrow().is_empty()));
            assert!(HOST_BOUNDARIES.with(|boundaries| boundaries.borrow().is_empty()));
            if matches!(outcome, Outcome::Panic) {
                assert!(runtime.is_poisoned());
                assert!(matches!(
                    context.eval("42"),
                    Err(crate::engine::api::RuntimeError::Poisoned)
                ));
                continue;
            }
            assert!(!runtime.is_poisoned());
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            assert_eq!(runtime.0.module_host_callback_depth.get(), 0);
            assert_eq!(context.eval("reenter()").unwrap(), Value::Int(41));
            assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
        }
    }
}

#[cfg(all(test, not(target_family = "wasm"), panic = "unwind"))]
mod cleanup_poison_tests {
    use super::{ExecutionLimits, RunningExecution};
    use crate::engine::{
        api::{Runtime, RuntimeError},
        code::runtime::PublishedFunctionSnapshot,
        value::JsValue,
        vm::{
            CallInput, Completion,
            closure::FrameFunction,
            frame::{ColdFrame, FrameCold, FrameEntry},
            frames::ActiveFrameToken,
            property_driver::{OwnedGetterSelection, SelectedNamedRead},
            stack::FrameStorage,
        },
    };

    #[test]
    fn failed_execution_owner_release_stops_before_frame_cleanup() {
        for case in ["pending", "completion", "selected-getter"] {
            let status = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "engine::vm::execution::cleanup_poison_tests::cleanup_child",
                    "--nocapture",
                ])
                .env("QJS_CLEANUP_POISON_CHILD", case)
                // Exercise the release-build path without a diagnostic panic.
                // The child alone gets this setting; parallel tests are isolated.
                .env("QJS_TEARDOWN_PROBE", "1")
                .status()
                .unwrap();
            assert!(status.success(), "cleanup subprocess {case}: {status}");
        }
    }

    #[test]
    fn cleanup_child() {
        let Ok(case) = std::env::var("QJS_CLEANUP_POISON_CHILD") else {
            return;
        };
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let frame_owner = runtime.new_object(None).unwrap().into_handle();
        let function = runtime.new_object(None).unwrap();
        let function_id = function.object_id();
        let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm_id());
        executable.metadata.max_stack = 1;
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        crate::engine::vm::driver::push_frame(
            &runtime,
            &mut execution,
            FrameEntry {
                initialize_bindings: false,
                property_generation: 0,
                iterator_generation: 0,
                caller_realm: context.realm_id(),
                active_frame: ActiveFrameToken::unmaterialized(),
                executable,
                cold: ColdFrame::new(FrameCold {
                    rare: Default::default(),
                    return_to: None,
                    entry_guard: None,
                    function: FrameFunction::new(function, Default::default())
                        .unwrap()
                        .into(),
                    reusable_captured_locals: Vec::new(),
                    input: CallInput::new(&runtime, JsValue::Undefined, JsValue::Undefined, None)
                        .into(),
                }),
                storage: FrameStorage {
                    original_arguments: Vec::new(),
                    parameters: Vec::new(),
                    locals: Vec::new(),
                    operands: vec![JsValue::Object(frame_owner)],
                },
            },
        )
        .unwrap();
        let stale = runtime.new_object(None).unwrap().into_handle();
        match case.as_str() {
            "pending" => execution.pending = Some(JsValue::Object(stale)),
            "completion" => {
                execution.pending_completion = Some(Completion::Throw(JsValue::Object(stale)));
            }
            "selected-getter" => {
                let mut state = runtime.0.state.borrow_mut();
                let selected = OwnedGetterSelection::prepare(
                    &mut state,
                    &runtime.0.poisoned,
                    &JsValue::Object(frame_owner),
                    stale,
                )
                .unwrap();
                execution.selected_named_read = Some(SelectedNamedRead::Getter(selected));
                // Remove the original edge, leaving the selected getter edge.
                state.release_jsvalue(JsValue::Object(stale)).unwrap();
            }
            _ => panic!("unknown cleanup subprocess"),
        }
        // Corrupt only the owner being released first. All later owners remain
        // valid; cleanup must stop instead of touching them after quarantine.
        runtime
            .0
            .state
            .borrow_mut()
            .release_jsvalue(JsValue::Object(stale))
            .unwrap();
        drop(execution);
        assert!(runtime.is_poisoned());
        let state = runtime.0.state.borrow();
        assert_eq!(state.heap.object_strong_count(function_id), Ok(1));
        assert_eq!(
            state.heap.object_strong_count(frame_owner),
            Ok(if case == "selected-getter" { 2 } else { 1 })
        );
        drop(state);
        assert!(matches!(runtime.new_context(), Err(RuntimeError::Poisoned)));
        assert_eq!(runtime.0.raw_execution_owners.get(), 0);
        drop(context);
        drop(runtime);
        assert!(super::ACTIVE_EXECUTIONS.with(|active| active.borrow().is_empty()));
    }
}
