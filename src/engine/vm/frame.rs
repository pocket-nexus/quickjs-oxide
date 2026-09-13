//! One executable owner and one exclusive storage window per running frame.

use crate::engine::api::error::Error;
use crate::engine::code::runtime::PublishedFunctionSnapshot;
use crate::engine::heap::ContextId;
use crate::engine::heap::roots::VarRefRoot;
use crate::engine::object::ObjectRef;
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
    pub frame: FrameId,
    pub tail: bool,
    pub operation: Option<OperationTarget>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum OperationTarget {
    Conversion(u64),
    PropertyGet(u64),
    Constructor(u64),
    ClassDefinition(u64),
    HasBinding(u64),
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
    pub regions: Vec<crate::engine::vm::VmUnwindRegion>,
    pub eval_arguments: Option<Vec<crate::engine::value::Value>>,
    pub has_binding_wait: Option<crate::engine::vm::with_driver::PendingHas>,
    pub class_wait: Option<crate::engine::vm::construct_driver::PendingClass>,
    pub constructor_wait: Option<crate::engine::vm::construct_driver::PendingConstructor>,
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

    pub(super) fn push(&mut self, frame: Frame) -> Result<FrameId, Error> {
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
        let id = FrameId {
            execution: self.execution,
            generation: self.next_generation,
        };
        self.next_generation = next;
        self.frames.push((id, frame));
        #[cfg(feature = "profiling")]
        {
            use crate::engine::api::profiling::{OwnedStorageEvent as Cost, record_owned_storage};
            record_owned_storage(Cost::FrameCapacity {
                before,
                after: self.frames.capacity(),
            });
            record_owned_storage(Cost::FramePush(self.frames.len()));
        }
        Ok(id)
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
            regions: Vec::new(),
            constructor_wait: None,
            class_wait: None,
            has_binding_wait: None,
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
}
