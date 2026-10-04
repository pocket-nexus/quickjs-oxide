use super::deferred::DeferredOperations;
use super::runtime::DeferredRefOp;
use crate::engine::api::Runtime;
use crate::engine::value::Value;

#[test]
fn state_queries_leave_deferred_releases_pending() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let id = object.object_id();
    let counts = runtime.0.state.borrow().heap.counts();
    {
        let _state = runtime.0.state.borrow();
        drop(object);
    }
    assert!(runtime.0.deferred_references.has_pending());

    assert_eq!(runtime.heap_counts().unwrap(), counts);
    assert!(runtime.0.deferred_references.has_pending());
    runtime.debug_info_mode().unwrap();
    assert!(runtime.0.deferred_references.has_pending());
    #[cfg(feature = "profiling")]
    {
        assert_eq!(runtime.memory_snapshot().unwrap().heap, counts);
        assert!(runtime.0.deferred_references.has_pending());
    }
    assert!(runtime.0.state.borrow().heap.object(id).is_ok());

    runtime.drain_deferred_references().unwrap();
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(runtime.0.state.borrow().heap.object(id).is_err());
}

fn restoration(depth: usize) -> DeferredRefOp {
    DeferredRefOp::ActiveCollectionRecordsTruncate { depth }
}

fn restoration_depth(operation: Option<DeferredRefOp>) -> usize {
    let Some(DeferredRefOp::ActiveCollectionRecordsTruncate { depth }) = operation else {
        panic!("missing restoration operation");
    };
    depth
}

#[test]
fn deferred_queue_keeps_priority_and_work_enqueued_during_a_drain() {
    let queue = DeferredOperations::default();
    assert!(!queue.has_pending());
    queue.push_back(restoration(1));
    queue.push_back(restoration(2));
    queue.push_front(restoration(3));
    let guard = queue.try_start_draining().unwrap();
    assert!(queue.try_start_draining().is_none());
    assert_eq!(restoration_depth(queue.pop_front()), 3);
    // Work created while applying the first operation must retain front priority.
    queue.push_front(restoration(4));
    assert_eq!(restoration_depth(queue.pop_front()), 4);
    assert_eq!(restoration_depth(queue.pop_front()), 1);
    assert_eq!(restoration_depth(queue.pop_front()), 2);
    assert!(!queue.has_pending());
    // Popping the last item does not finish the active drainer: its application
    // may still create more work before the guard is dropped.
    queue.push_back(restoration(5));
    assert!(queue.has_pending());
    assert!(queue.try_start_draining().is_none());
    assert_eq!(restoration_depth(queue.pop_front()), 5);
    drop(guard);
    assert!(!queue.has_pending());
    assert!(queue.try_start_draining().is_some());
}

#[test]
fn deferred_drain_guard_resets_after_unwinding_without_losing_pending_work() {
    let queue = DeferredOperations::default();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = queue.try_start_draining().unwrap();
        queue.push_back(restoration(7));
        panic!("simulated unwind during cleanup");
    }));
    assert!(result.is_err());
    assert!(queue.has_pending());
    let _guard = queue.try_start_draining().unwrap();
    assert_eq!(restoration_depth(queue.pop_front()), 7);
    assert!(!queue.has_pending());
}

#[test]
fn idle_and_borrow_blocked_checkpoints_leave_the_queue_untouched() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let id = object.object_id();
    let state = runtime.0.state.borrow_mut();
    {
        let _queue_read = runtime.0.deferred_references.borrow();
        // Both borrows deliberately forbid a mutable borrow in the idle path.
        runtime.drain_deferred_references().unwrap();
    }
    drop(object);
    assert!(runtime.0.deferred_references.has_pending());
    {
        let queue_read = runtime.0.deferred_references.borrow();
        assert_eq!(queue_read.len(), 1);
        runtime.drain_deferred_references().unwrap();
        assert_eq!(queue_read.len(), 1);
    }
    assert!(state.heap.object(id).is_ok());
    drop(state);
    // The existing operation boundary, including nested boundaries, still drains.
    let _operation = runtime.operation().unwrap();
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(runtime.0.state.borrow().heap.object(id).is_err());
}

#[test]
fn failed_deferred_operation_quarantines_remaining_work() {
    let runtime = Runtime::new();
    let stale = runtime.new_object(None).unwrap();
    let stale_id = stale.object_id();
    drop(stale);
    let live = runtime.new_object(None).unwrap();
    let live_id = live.object_id();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(stale_id));
    let state = runtime.0.state.borrow();
    drop(live);
    drop(state);
    assert!(runtime.drain_deferred_references().is_err());
    assert!(runtime.0.deferred_references.has_pending());
    assert!(runtime.0.state.borrow().heap.object(live_id).is_ok());
    assert!(runtime.is_poisoned());
    assert_eq!(
        runtime.drain_deferred_references(),
        Err(crate::engine::api::RuntimeError::Poisoned)
    );
    assert!(runtime.0.deferred_references.has_pending());
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(live_id),
        Ok(1)
    );
    // Failed servicing releases its queue lease even though state is now
    // quarantined. No heap operation may consume the remaining work.
    assert!(runtime.0.deferred_references.try_start_draining().is_some());
}

#[test]
fn cascading_zero_reference_destruction_finishes_before_release_returns() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    // Warm the ordinary object path before recording the persistent baseline.
    drop(context.eval("({ next: null })").unwrap());
    let before = runtime.0.state.borrow().heap.counts().object_nodes;
    let root = context
        .eval("(() => { let root = null; for (let i = 0; i < 10000; i++) root = { next: root }; return root; })()")
        .unwrap();
    assert!(matches!(root, Value::Object(_)));
    assert_eq!(
        runtime.0.state.borrow().heap.counts().object_nodes,
        before + 10000
    );
    drop(root);
    assert_eq!(runtime.0.state.borrow().heap.counts().object_nodes, before);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn runtime_teardown_applies_queued_bytecode_context_and_atom_releases() {
    let runtime = Runtime::new();
    let weak = std::rc::Rc::downgrade(&runtime.0);
    let mut context = runtime.new_context().expect("create context");
    let bytecode = context.compile("({ value: 42 })").unwrap();
    let key = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::QueuedAtTeardown)
        .unwrap();
    let state = runtime.0.state.borrow_mut();
    drop(bytecode);
    drop(context);
    drop(key);
    assert!(runtime.0.deferred_references.has_pending());
    drop(state);
    // No intervening safe point: RuntimeInner::drop must use the same operation
    // application rules and satisfy its zero-live-node teardown assertion.
    drop(runtime);
    assert!(weak.upgrade().is_none());
}

#[test]
fn live_heap_reference_release_has_no_runtime_cleanup_payload() {
    use super::{Heap, ObjectData, RawId};
    use crate::engine::object::shape::Shape;

    let mut heap = Heap::new();
    let shape = heap.allocate_shape(Shape::new(None, []).unwrap()).unwrap();
    let object = heap
        .allocate_object(ObjectData::ordinary(shape, Vec::new()))
        .unwrap();
    heap.release_shape(shape).unwrap();
    heap.retain_object(object).unwrap();
    assert!(
        heap.release_reference(RawId::Object(object))
            .unwrap()
            .is_none()
    );
    assert_eq!(heap.object_strong_count(object).unwrap(), 1);
    let cleanup = heap
        .release_reference(RawId::Object(object))
        .unwrap()
        .unwrap();
    assert_eq!(cleanup.finalized_objects, 1);
    assert_eq!(cleanup.finalized_shapes, 1);
    assert_eq!(heap.counts().live, 0);
    assert!(heap.release_reference(RawId::Object(object)).is_err());
}

#[test]
fn nonzero_release_still_drains_previously_queued_nodes() {
    use super::{Heap, ObjectData, RawId};
    use crate::engine::object::shape::Shape;

    let mut heap = Heap::new();
    let shape = heap.allocate_shape(Shape::new(None, []).unwrap()).unwrap();
    let queued = heap
        .allocate_object(ObjectData::ordinary(shape, Vec::new()))
        .unwrap();
    let retained = heap
        .allocate_object(ObjectData::ordinary(shape, Vec::new()))
        .unwrap();
    heap.release_shape(shape).unwrap();
    heap.retain_object(retained).unwrap();
    heap.release_raw_no_drain(RawId::Object(queued)).unwrap();
    assert!(!heap.zero_queue.is_empty());
    let cleanup = heap
        .release_reference(RawId::Object(retained))
        .unwrap()
        .unwrap();
    assert_eq!(cleanup.finalized_objects, 1);
    assert_eq!(cleanup.finalized_shapes, 0);
    assert!(heap.object(queued).is_err());
    assert_eq!(heap.object_strong_count(retained).unwrap(), 1);
    assert!(heap.zero_queue.is_empty());
    // The existing public API still returns the full cleanup counters.
    let cleanup = heap.release_object(retained).unwrap();
    assert_eq!(cleanup.finalized_objects, 1);
    assert_eq!(cleanup.finalized_shapes, 1);
    assert_eq!(heap.counts().live, 0);
}

#[test]
fn arguments_prefix_snapshot_preserves_pending_zero_cleanup_service_policy() {
    use super::RawId;
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let Value::Object(carrier) = context.eval("(function(){return arguments})(1,2)").unwrap()
    else {
        panic!("carrier")
    };
    let pending = runtime.new_object(None).unwrap().into_handle();
    let checkpoint = runtime.new_object(None).unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .release_raw_no_drain(RawId::Object(pending))
        .unwrap();
    assert!(runtime.0.state.borrow().heap.has_pending_zero_cleanup());
    assert!(
        runtime
            .prepare_fast_array_arguments_jsvalue(context.realm, &carrier)
            .unwrap()
            .is_some()
    );
    assert!(runtime.0.state.borrow().heap.has_pending_zero_cleanup());
    drop(checkpoint);
    assert!(!runtime.0.state.borrow().heap.has_pending_zero_cleanup());
    assert!(runtime.0.state.borrow().heap.object(pending).is_err());
}

#[test]
fn arguments_prefix_only_promotes_actual_outputs_at_carrier_saturation() {
    use crate::engine::heap::RawId;
    use crate::engine::value::{JsValue, conversion::NativeConversion};
    for count in [u32::MAX - 3, u32::MAX - 2, u32::MAX - 1, u32::MAX] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(carrier) = context.eval("(function(){return arguments})(1)").unwrap()
        else {
            panic!()
        };
        let id = carrier.object_id();
        let actual = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(id)
            .unwrap();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), count);
        let result = runtime.prepare_fast_array_arguments_jsvalue(context.realm, &carrier);
        let after = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(id)
            .unwrap();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), actual);
        let Some(NativeConversion::Value(values)) = result.unwrap() else {
            panic!("actual immediate output requires no carrier retain")
        };
        assert!(matches!(values.as_slice(), [JsValue::Int(1)]));
        assert_eq!(after, count);
        for value in values {
            runtime.release_jsvalue(value).unwrap();
        }
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn arguments_prefix_checks_each_self_alias_output_and_preserves_pinned_length() {
    use crate::engine::atom::pinned::PinnedAtom;
    use crate::engine::heap::RawId;
    use crate::engine::value::{JsValue, conversion::NativeConversion};
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let Value::Object(carrier) = context
        .eval("(function(a){arguments[0]=arguments;return arguments})(1)")
        .unwrap()
    else {
        panic!("carrier");
    };
    let id = carrier.object_id();
    let original_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    let key = runtime.pinned_property_key(PinnedAtom::Length).unwrap();
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .atoms
            .resolve(key.atom())
            .unwrap()
            .ref_count,
        None
    );
    drop(key);
    for count in [u32::MAX - 4, u32::MAX - 3, u32::MAX - 2] {
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), count);
        let result = runtime
            .prepare_fast_array_arguments_jsvalue(context.realm, &carrier)
            .unwrap();
        match result {
            Some(NativeConversion::Value(values)) => {
                assert!(matches!(values.as_slice(), [JsValue::Object(value)] if *value == id));
                assert_eq!(
                    runtime
                        .0
                        .state
                        .borrow()
                        .heap
                        .object_strong_count(id)
                        .unwrap(),
                    count + 1
                );
                for value in values {
                    runtime.release_jsvalue(value).unwrap();
                }
            }
            None => panic!("intact Arguments does not require intermediate carrier headroom"),
            Some(NativeConversion::Throw(_)) => panic!("unexpected throw"),
        }
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(id)
                .unwrap(),
            count
        );
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), original_count);
    }
    drop(carrier);
}
