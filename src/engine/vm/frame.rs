//! One executable owner and one exclusive storage window per running frame.

use crate::engine::api::error::Error;
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::ContextId;
use crate::engine::heap::roots::VarRefRoot;
use crate::engine::object::ObjectRef;
use crate::engine::value::Value;
use crate::engine::vm::CallInput;
use crate::engine::vm::frames::{ActiveFrameGuard, ActiveFrameToken};
use crate::engine::vm::stack::{FrameStorage, FrameWindow};

#[derive(Clone, Copy)]
pub(super) enum ReturnValue {
    Push,
    Discard,
}

#[derive(Clone, Copy)]
pub(super) struct ReturnTarget {
    pub value_use: ReturnValue,
    pub owner: ReturnOwner,
    pub tail: bool,
    pub operation: Option<OperationTarget>,
}

/// A continuation may belong to a bytecode frame or a root native/job request.
#[derive(Clone, Copy)]
pub(super) enum ReturnOwner {
    Frame(FrameId),
    Root,
}
impl ReturnOwner {
    pub(super) fn frame(self) -> Result<FrameId, Error> {
        match self {
            Self::Frame(id) => Ok(id),
            Self::Root => Err(Error::internal(
                "root request used a bytecode-only operation",
            )),
        }
    }
}
impl ReturnTarget {
    pub(super) fn frame(self) -> Result<FrameId, Error> {
        self.owner.frame()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OperationTarget {
    Conversion(u64),
    PropertyGet(u64),
    Iterator(u64),
    Eval(u16),
}

pub(super) enum ConstructorReturn {
    Base(crate::engine::value::Value),
    Derived,
}

pub(super) struct FrameCold {
    pub property_wait: Option<Box<super::proxy_get_driver::PendingProxyGet>>,
    pub property_generation: u64,
    pub iterator_generation: u64,
    pub iterator_wait: Option<Box<crate::engine::vm::iterator_driver::PendingIterator>>,
    pub resume_throw: Option<Value>,
    pub regions: Vec<crate::engine::vm::VmUnwindRegion>,
    pub eval_arguments: Option<Vec<crate::engine::value::Value>>,
    pub constructor_return: Option<ConstructorReturn>,
    pub conversion: Option<crate::engine::vm::conversion_driver::ConversionWait>,
    pub normalized_this: Option<crate::engine::value::Value>,
    pub return_to: Option<ReturnTarget>,
    pub entry_guard: Option<ActiveFrameGuard>,
    pub caller_realm: ContextId,
    pub active_frame: ActiveFrameToken,
    pub function: ObjectRef,
    pub closure_slots: Vec<VarRefRoot>,
    pub reusable_captured_locals: Vec<bool>,
    pub input: CallInput,
}

/// Owners crossing the driver boundary before installation or after detachment.
pub(super) struct FrameEntry {
    pub executable: PublishedFunctionSnapshot,
    pub cold: Box<FrameCold>,
    pub storage: FrameStorage,
}

pub(super) struct Frame {
    pub executable: PublishedFunctionSnapshot,
    pub window: FrameWindow,
    pub fault_pc: usize,
    pub resume_pc: usize,
    pub cold: Box<FrameCold>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FrameId {
    execution: u64,
    generation: u64,
}

pub(super) struct FrameStore {
    frames: Vec<(FrameId, Frame)>,
    execution: u64,
    next_generation: u64,
    limit: usize,
}

impl FrameStore {
    pub(super) fn new(execution: u64, limit: usize) -> Self {
        Self {
            frames: Vec::new(),
            execution,
            next_generation: 1,
            limit,
        }
    }

    pub(super) fn can_push(&self) -> bool {
        self.frames.len() < self.limit
    }

    /// Domain continuations replace recursive calls and share the existing
    /// execution depth ceiling, including calls that install no bytecode frame.
    pub(super) fn can_push_with_continuations(&self, pending: usize) -> bool {
        self.frames
            .len()
            .checked_add(pending)
            .and_then(|depth| {
                self.frames.iter().try_fold(depth, |depth, (_, frame)| {
                    depth.checked_add(
                        frame
                            .cold
                            .property_wait
                            .as_ref()
                            .map_or(0, |wait| wait.continuation_depth()),
                    )
                })
            })
            .is_some_and(|depth| depth < self.limit)
    }

    pub(super) fn current_id(&self) -> Option<FrameId> {
        self.frames.last().map(|(id, _)| *id)
    }

    /// Reserve identity and capacity while retaining exclusive access to this
    /// store. Slot publication may then fail, but installing a frame cannot.
    pub(super) fn prepare_push(&mut self) -> Result<FramePush<'_>, Error> {
        if self.frames.len() >= self.limit {
            return Err(Error::internal("execution frame limit exceeded"));
        }
        let next = self
            .next_generation
            .checked_add(1)
            .ok_or_else(|| Error::internal("execution frame identity exhausted"))?;
        #[cfg(feature = "profiling")]
        let before = self.frames.capacity();
        self.frames
            .try_reserve(1)
            .map_err(|_| Error::internal("execution frame allocation failed"))?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_storage(
            crate::engine::api::profiling::OwnedStorageEvent::FrameCapacity {
                before,
                after: self.frames.capacity(),
            },
        );
        Ok(FramePush { store: self, next })
    }

    #[cfg(test)]
    pub(super) fn push(&mut self, frame: Frame) -> Result<FrameId, Error> {
        Ok(self.prepare_push()?.install(frame))
    }

    pub(super) fn pop_current(&mut self) -> Option<Frame> {
        self.frames.pop().map(|(_, frame)| frame)
    }

    pub(super) fn current_mut(&mut self, id: FrameId) -> Result<&mut Frame, Error> {
        match self.frames.last_mut() {
            Some((current, frame)) if *current == id => Ok(frame),
            _ => Err(Error::internal(
                "frame identity is not the current execution frame",
            )),
        }
    }

    pub(super) fn pop(&mut self, id: FrameId) -> Result<Frame, Error> {
        self.current_mut(id)?;
        Ok(self.frames.pop().unwrap().1)
    }
}

/// The borrow prevents another push/pop from invalidating reserved capacity.
pub(super) struct FramePush<'a> {
    store: &'a mut FrameStore,
    next: u64,
}
impl FramePush<'_> {
    pub(super) fn install(self, frame: Frame) -> FrameId {
        let id = FrameId {
            execution: self.store.execution,
            generation: self.store.next_generation,
        };
        self.store.next_generation = self.next;
        self.store.frames.push((id, frame));
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_storage(
            crate::engine::api::profiling::OwnedStorageEvent::FramePush(self.store.frames.len()),
        );
        id
    }
}
impl Drop for FrameStore {
    fn drop(&mut self) {
        // Child query/activation owners must disappear before their parents.
        while let Some(frame) = self.pop_current() {
            drop(frame);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::api::Runtime;
    use crate::engine::value::Value;
    use crate::engine::vm::stack::{FrameStorage, SlotStore};

    fn frame(runtime: &Runtime, realm: ContextId) -> (Frame, SlotStore) {
        let executable = PublishedFunctionSnapshot::empty_for_test(realm);
        let mut slots = SlotStore::new(0);
        let window = slots
            .push_frame(
                &executable.frame_layout(),
                FrameStorage {
                    original_arguments: Vec::new(),
                    parameters: Vec::new(),
                    locals: Vec::new(),
                    operands: Vec::new(),
                },
            )
            .unwrap();
        let function = runtime.new_object(None).unwrap();
        let cold = Box::new(FrameCold {
            resume_throw: None,
            regions: Vec::new(),
            iterator_wait: None,
            property_wait: None,
            property_generation: 0,
            iterator_generation: 0,
            eval_arguments: None,
            constructor_return: None,
            conversion: None,
            normalized_this: None,
            return_to: None,
            entry_guard: None,
            caller_realm: realm,
            active_frame: ActiveFrameToken(0),
            input: CallInput {
                this_value: Value::Undefined,
                new_target: Value::Undefined,
                callee_global: function.clone(),
            },
            function,
            closure_slots: Vec::new(),
            reusable_captured_locals: Vec::new(),
        });
        (
            Frame {
                executable,
                window,
                fault_pc: 0,
                resume_pc: 0,
                cold,
            },
            slots,
        )
    }

    #[test]
    fn limits_and_stale_ids_preserve_the_active_frame_and_release_rejected_owners() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut frames = FrameStore::new(1, 1);
        let (first, _first_slots) = frame(&runtime, context.realm);
        let first_id = frames.push(first).unwrap();
        let capacity = frames.frames.capacity();
        let (rejected, _rejected_slots) = frame(&runtime, context.realm);
        let rejected_object = rejected.cold.function.object_id();
        assert!(frames.push(rejected).is_err());
        assert!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object(rejected_object)
                .is_err()
        );
        assert!(frames.current_mut(first_id).is_ok());
        assert_eq!(frames.frames.capacity(), capacity);
        drop(frames.pop(first_id).unwrap());
        let (replacement, _replacement_slots) = frame(&runtime, context.realm);
        let replacement_id = frames.push(replacement).unwrap();
        assert_ne!(first_id, replacement_id);
        assert!(frames.current_mut(first_id).is_err());
        assert!(
            frames
                .current_mut(FrameId {
                    execution: 2,
                    generation: replacement_id.generation
                })
                .is_err()
        );
        assert!(frames.current_mut(replacement_id).is_ok());
        assert_eq!(frames.frames.capacity(), capacity);
    }

    #[test]
    fn exhausted_frame_identity_rejects_before_installing_ownership() {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let mut frames = FrameStore::new(1, 1);
        frames.next_generation = u64::MAX;
        let (rejected, _slots) = frame(&runtime, context.realm);
        let object = rejected.cold.function.object_id();
        assert!(frames.push(rejected).is_err());
        assert!(frames.frames.is_empty());
        assert_eq!(frames.frames.capacity(), 0);
        assert!(runtime.0.state.borrow().heap.object(object).is_err());
    }
    fn entry(runtime: &Runtime, realm: ContextId) -> FrameEntry {
        let (frame, _slots) = frame(runtime, realm);
        FrameEntry {
            executable: frame.executable,
            cold: frame.cold,
            storage: FrameStorage {
                original_arguments: Vec::new(),
                parameters: Vec::new(),
                locals: Vec::new(),
                operands: Vec::new(),
            },
        }
    }

    #[test]
    fn rejected_child_push_preserves_parent_window_and_releases_child_owners() {
        use crate::engine::vm::{
            driver::push_frame,
            execution::{ExecutionLimits, RunningExecution},
        };
        for exhausted_identity in [false, true] {
            let runtime = Runtime::new();
            let context = runtime.new_context();
            let mut execution = RunningExecution::new(
                &runtime,
                ExecutionLimits {
                    frames: 1,
                    slots: 16,
                },
            )
            .unwrap();
            let parent = push_frame(&mut execution, entry(&runtime, context.realm)).unwrap();
            let generation = execution.frames.next_generation;
            if exhausted_identity {
                execution.frames.limit = 2;
                execution.frames.next_generation = u64::MAX;
            }
            let mut child = entry(&runtime, context.realm);
            let child_object = child.cold.function.object_id();
            child.storage.original_arguments.push(Value::Int(42));
            child
                .storage
                .parameters
                .push(super::super::bindings::FrameBinding::Direct(Value::Int(42)));
            let error = push_frame(&mut execution, child).unwrap_err();
            assert!(error.to_string().contains(if exhausted_identity {
                "identity exhausted"
            } else {
                "frame limit"
            }));
            assert_eq!(execution.frames.current_id(), Some(parent));
            let frame = execution.frames.current_mut(parent).unwrap();
            assert_eq!(
                execution.slots.binding_counts(&frame.window).unwrap(),
                (0, 0)
            );
            assert!(runtime.0.state.borrow().heap.object(child_object).is_err());
            execution.frames.limit = 2;
            execution.frames.next_generation = generation;
            let replacement = push_frame(&mut execution, entry(&runtime, context.realm)).unwrap();
            let frame = execution.frames.pop(replacement).unwrap();
            execution.slots.clear_frame(frame.window).unwrap();
            let parent = execution.frames.current_mut(parent).unwrap();
            assert_eq!(
                execution.slots.binding_counts(&parent.window).unwrap(),
                (0, 0)
            );
        }
    }

    struct DropLog(
        &'static str,
        std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    );
    impl Drop for DropLog {
        fn drop(&mut self) {
            self.1.borrow_mut().push(self.0);
        }
    }
    fn tracked_runtime(
        name: &'static str,
        events: &std::rc::Rc<std::cell::RefCell<Vec<&'static str>>>,
    ) -> Runtime {
        let runtime = Runtime::new();
        let log = DropLog(name, events.clone());
        runtime.set_host_promise_rejection_tracker(move |_| {
            let _ = &log;
        });
        runtime
    }
    #[test]
    fn frame_store_abandon_releases_inner_runtime_owner_first() {
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut frames = FrameStore::new(1, 2);
        for name in ["parent", "child"] {
            let runtime = tracked_runtime(name, &events);
            let context = runtime.new_context();
            let (frame, _slots) = frame(&runtime, context.realm);
            frames.push(frame).unwrap();
        }
        assert!(events.borrow().is_empty());
        drop(frames);
        assert_eq!(*events.borrow(), ["child", "parent"]);
    }
    #[test]
    fn execution_abandon_clears_child_slots_before_parent_frame_owner() {
        use crate::engine::vm::{
            driver::push_frame,
            execution::{ExecutionLimits, RunningExecution},
        };
        let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let parent_runtime = tracked_runtime("parent", &events);
        let parent_context = parent_runtime.new_context();
        let mut execution = RunningExecution::new(
            &parent_runtime,
            ExecutionLimits {
                frames: 2,
                slots: 16,
            },
        )
        .unwrap();
        push_frame(&mut execution, entry(&parent_runtime, parent_context.realm)).unwrap();
        {
            let runtime = tracked_runtime("child", &events);
            let context = runtime.new_context();
            let captured_runtime = tracked_runtime("child-slot", &events);
            let capture = captured_runtime.new_object(None).unwrap();
            let mut child = entry(&runtime, context.realm);
            child
                .storage
                .original_arguments
                .push(Value::Object(capture));
            child
                .storage
                .parameters
                .push(super::super::bindings::FrameBinding::Direct(
                    Value::Undefined,
                ));
            push_frame(&mut execution, child).unwrap();
        }
        drop(parent_context);
        drop(parent_runtime);
        assert!(events.borrow().is_empty());
        drop(execution);
        assert_eq!(*events.borrow(), ["child-slot", "child", "parent"]);
    }
}
