//! Narrow owning-slot reference transactions used by the stack interpreter.
//! Readiness never changes a reference count. The ready path cannot allocate,
//! drain the zero queue, run deferred work or invoke JavaScript. A boundary
//! result leaves the original Value intact for the driver to handle once.

use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;
use crate::engine::heap::{Heap, HeapError, RawId, SlotState};
use crate::engine::value::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SlotReleaseReadiness {
    Ready,
    QueueCapacity,
    Drain,
    Deferred,
    Borrowed,
    PrimitiveStorage,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::value::{JsString, bigint::JsBigInt};

    #[test]
    fn last_root_preflight_preserves_ownership_until_driver_release() {
        let runtime = Runtime::new();
        let object = runtime.new_object(None).unwrap();
        let id = RawId::Object(object.object_id());
        let mut value = Value::Object(object);
        // Force the two capacity cases independently of allocator reuse.
        runtime.0.state.borrow_mut().heap.zero_queue.shrink_to_fit();
        for _ in 0..2 {
            assert_eq!(
                runtime.slot_value_release_readiness(&value).unwrap(),
                SlotReleaseReadiness::QueueCapacity
            );
            assert!(!runtime.try_release_slot_value(&mut value).unwrap());
            assert_eq!(runtime.0.state.borrow().heap.strong_count(id).unwrap(), 1);
        }
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .zero_queue
            .try_reserve(1)
            .unwrap();
        for _ in 0..2 {
            assert_eq!(
                runtime.slot_value_release_readiness(&value).unwrap(),
                SlotReleaseReadiness::Drain
            );
            assert!(!runtime.try_release_slot_value(&mut value).unwrap());
            assert_eq!(runtime.0.state.borrow().heap.strong_count(id).unwrap(), 1);
        }
        drop(value);
        assert!(runtime.0.state.borrow().heap.strong_count(id).is_err());
    }

    #[test]
    fn shared_root_release_commits_once_and_keeps_other_owner_alive() {
        let runtime = Runtime::new();
        let root = runtime.new_object(None).unwrap();
        let id = RawId::Object(root.object_id());
        let mut value = Value::Object(root.try_clone().unwrap());
        assert_eq!(runtime.0.state.borrow().heap.strong_count(id).unwrap(), 2);
        for _ in 0..2 {
            assert!(runtime.try_release_slot_value(&mut value).unwrap());
            assert_eq!(value, Value::Undefined);
            assert_eq!(runtime.0.state.borrow().heap.strong_count(id).unwrap(), 1);
            assert!(runtime.0.state.borrow().heap.zero_queue.is_empty());
        }
        drop(root);
        assert!(runtime.0.state.borrow().heap.strong_count(id).is_err());
    }

    #[test]
    fn blocked_borrow_and_deferred_release_do_not_commit_or_drain() {
        let runtime = Runtime::new();
        let root = runtime.new_object(None).unwrap();
        let id = RawId::Object(root.object_id());
        let mut value = Value::Object(root.try_clone().unwrap());
        let deferred = root.try_clone().unwrap();
        {
            let state = runtime.0.state.borrow();
            assert_eq!(
                runtime.slot_value_release_readiness(&value).unwrap(),
                SlotReleaseReadiness::Borrowed
            );
            assert!(!runtime.try_release_slot_value(&mut value).unwrap());
            assert!(!runtime.0.deferred_references.has_pending());
            assert_eq!(state.heap.strong_count(id).unwrap(), 3);
            drop(deferred);
        }
        for _ in 0..2 {
            assert_eq!(
                runtime.slot_value_release_readiness(&value).unwrap(),
                SlotReleaseReadiness::Deferred
            );
            assert!(!runtime.try_release_slot_value(&mut value).unwrap());
            assert_eq!(runtime.0.state.borrow().heap.strong_count(id).unwrap(), 3);
            assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
        }
        runtime.drain_deferred_references().unwrap();
        assert!(runtime.try_release_slot_value(&mut value).unwrap());
        assert_eq!(runtime.0.state.borrow().heap.strong_count(id).unwrap(), 1);
    }

    #[test]
    fn existing_zero_queue_is_not_drained_by_a_shared_slot_release() {
        let runtime = Runtime::new();
        let root = runtime.new_object(None).unwrap();
        let id = RawId::Object(root.object_id());
        let mut value = Value::Object(root.try_clone().unwrap());
        let queued = runtime.new_object(None).unwrap();
        let queued_id = queued.object_id();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .retain_object(queued_id)
            .unwrap();
        drop(queued);
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .release_raw_no_drain(RawId::Object(queued_id))
            .unwrap();
        assert_eq!(
            runtime.slot_value_release_readiness(&value).unwrap(),
            SlotReleaseReadiness::Drain
        );
        assert!(!runtime.try_release_slot_value(&mut value).unwrap());
        {
            let mut state = runtime.0.state.borrow_mut();
            assert_eq!(state.heap.strong_count(id).unwrap(), 2);
            assert_eq!(state.heap.zero_queue.len(), 1);
            let cleanup = state.heap.drain_zero_queue().unwrap();
            state.apply_cleanup(cleanup).unwrap();
        }
        assert!(runtime.try_release_slot_value(&mut value).unwrap());
        assert_eq!(runtime.0.state.borrow().heap.strong_count(id).unwrap(), 1);
    }

    #[test]
    fn primitive_backing_storage_is_only_released_at_a_boundary() {
        let runtime = Runtime::new();
        for original in [
            Value::String(JsString::from_static("slot")),
            Value::BigInt(JsBigInt::from(i128::MAX)),
        ] {
            let mut shared = original.clone();
            assert!(runtime.try_release_slot_value(&mut shared).unwrap());
            let mut last = original;
            assert_eq!(
                runtime.slot_value_release_readiness(&last).unwrap(),
                SlotReleaseReadiness::PrimitiveStorage
            );
            assert!(!runtime.try_release_slot_value(&mut last).unwrap());
        }
        let mut short = Value::BigInt(JsBigInt::from(1));
        assert!(runtime.try_release_slot_value(&mut short).unwrap());
    }

    #[test]
    fn retain_overflow_leaves_the_source_count_unchanged() {
        let runtime = Runtime::new();
        let root = runtime.new_object(None).unwrap();
        let id = RawId::Object(root.object_id());
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .live_node_mut(id)
            .unwrap()
            .strong = u32::MAX;
        let result = root.try_clone();
        let count = runtime.0.state.borrow().heap.strong_count(id).unwrap();
        // Restore the actual owner count before assertions can unwind roots.
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .live_node_mut(id)
            .unwrap()
            .strong = 1;
        assert!(result.is_err());
        assert_eq!(count, u32::MAX);
        drop(root);
        assert!(runtime.0.state.borrow().heap.strong_count(id).is_err());
    }

    #[test]
    fn foreign_root_is_rejected_without_changing_ownership() {
        let runtime = Runtime::new();
        let foreign = Runtime::new();
        let root = foreign.new_object(None).unwrap();
        let id = RawId::Object(root.object_id());
        let mut value = Value::Object(root);
        assert!(matches!(
            runtime.try_release_slot_value(&mut value),
            Err(RuntimeError::WrongRuntime(_))
        ));
        assert!(matches!(value, Value::Object(_)));
        assert_eq!(foreign.0.state.borrow().heap.strong_count(id).unwrap(), 1);
    }
}

impl Heap {
    fn slot_release_readiness(&self, id: RawId) -> Result<SlotReleaseReadiness, HeapError> {
        let index = self.validate_slot_identity(id)?;
        if !self.zero_queue.is_empty() {
            return Ok(SlotReleaseReadiness::Drain);
        }
        match &self.slots[index].state {
            SlotState::Live(node) if node.strong > 1 => Ok(SlotReleaseReadiness::Ready),
            SlotState::Live(node) if node.strong == 1 => {
                // release_raw_no_drain would push to this queue. Do not commit
                // its decrement before deciding whether that push can allocate.
                Ok(if self.zero_queue.len() == self.zero_queue.capacity() {
                    SlotReleaseReadiness::QueueCapacity
                } else {
                    SlotReleaseReadiness::Drain
                })
            }
            // Zombie reclamation and in-progress heap transitions also require
            // a driver boundary; they do not satisfy the ordinary live proof.
            _ => Ok(SlotReleaseReadiness::Drain),
        }
    }
}

impl Runtime {
    pub(crate) fn slot_value_release_readiness(
        &self,
        value: &Value,
    ) -> Result<SlotReleaseReadiness, RuntimeError> {
        match value {
            Value::Undefined | Value::Null | Value::Bool(_) | Value::Int(_) | Value::Float(_) => {
                return Ok(SlotReleaseReadiness::Ready);
            }
            Value::String(value) => {
                return Ok(if value.release_keeps_storage_alive() {
                    SlotReleaseReadiness::Ready
                } else {
                    SlotReleaseReadiness::PrimitiveStorage
                });
            }
            Value::BigInt(value) => {
                return Ok(if value.release_keeps_storage_alive() {
                    SlotReleaseReadiness::Ready
                } else {
                    SlotReleaseReadiness::PrimitiveStorage
                });
            }
            Value::Object(root) if !root.belongs_to(self) => {
                return Err(RuntimeError::WrongRuntime("owned object slot"));
            }
            Value::Symbol(root) if !root.belongs_to(self) => {
                return Err(RuntimeError::WrongRuntime("owned symbol slot"));
            }
            Value::Object(_) | Value::Symbol(_) => {}
        }
        if self.0.deferred_references.has_pending() {
            return Ok(SlotReleaseReadiness::Deferred);
        }
        // A successful shared borrow would not prove that Drop can acquire
        // its mutable runtime borrow. Acquire that exact permission here.
        let Ok(state) = self.0.state.try_borrow_mut() else {
            return Ok(SlotReleaseReadiness::Borrowed);
        };
        match value {
            Value::Object(root) => Ok(state
                .heap
                .slot_release_readiness(RawId::Object(root.object_id()))?),
            Value::Symbol(root) => Ok(match state.atoms.resolve(root.atom())?.ref_count {
                None => SlotReleaseReadiness::Ready,
                Some(count) if count > 1 => SlotReleaseReadiness::Ready,
                Some(_) => SlotReleaseReadiness::PrimitiveStorage,
            }),
            _ => unreachable!("primitive slots returned before borrowing runtime state"),
        }
    }

    /// Commit exactly one ordinary owning-root release after the no-drain
    /// proof. No callback or reference decrease can intervene between the
    /// preflight and Drop. Ready consumes the Value; every other outcome leaves
    /// it untouched, so the caller may move it to a pending operation safely.
    pub(crate) fn try_release_slot_value(&self, value: &mut Value) -> Result<bool, RuntimeError> {
        if self.slot_value_release_readiness(value)? != SlotReleaseReadiness::Ready {
            return Ok(false);
        }
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_storage(
            crate::engine::api::profiling::OwnedStorageEvent::HotRelease {
                heap_root: matches!(value, Value::Object(_) | Value::Symbol(_)),
            },
        );
        let old = std::mem::replace(value, Value::Undefined);
        drop(old);
        Ok(true)
    }
}
