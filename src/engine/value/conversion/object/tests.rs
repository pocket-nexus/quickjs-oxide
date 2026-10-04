//! ToObject retains its prototype before admission and owns rejected inputs explicitly.
use super::*;
use crate::engine::{
    api::Value,
    heap::{HeapError, ObjectPayload, RawId, runtime::DeferredRefOp},
    value::JsString,
};

fn string(runtime: &Runtime) -> (JsValue, crate::engine::heap::StringId) {
    let value = runtime
        .into_jsvalue(Value::String(JsString::from_static("boxed")))
        .unwrap();
    let JsValue::String(id) = value else {
        unreachable!()
    };
    (JsValue::String(id), id)
}
fn queue_rejected_admission(runtime: &Runtime) -> ObjectId {
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    runtime.release_jsvalue(JsValue::Object(first)).unwrap();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(first));
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(later));
    later
}

#[test]
fn to_object_state_preserves_original_object_and_boxes_against_actual_realm_without_runtime_owner()
{
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap().into_handle();
    let (input, string) = string(&runtime);
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let prototype = state
        .primitive_prototype_id_for_realm(context.realm, PrimitiveKind::String)
        .unwrap();
    let prototype_count = state.heap.object_strong_count(prototype).unwrap();
    let prototype_shapes = |state: &RuntimeState| {
        state
            .shape_hashes
            .keys()
            .filter(|id| {
                matches!(state.heap.shape(**id), Ok(shape) if shape.prototype() == Some(prototype))
            })
            .count() as u32
    };
    let initial_shapes = prototype_shapes(&state);
    let ToObjectOutcome::Existing(result) = state
        .native_to_object_jsvalue(&runtime.0.poisoned, context.realm, JsValue::Object(object))
        .unwrap()
    else {
        panic!("existing object is not a collectible producer")
    };
    assert_eq!(result, object);
    assert_eq!(state.heap.object_strong_count(object), Ok(1));
    let ToObjectOutcome::Boxed(wrapper) = state
        .native_to_object_jsvalue(&runtime.0.poisoned, context.realm, input)
        .unwrap()
    else {
        panic!("actual primitive wrapper producer")
    };
    let data = state.heap.object(wrapper).unwrap();
    assert_eq!(
        state.heap.shape(data.shape).unwrap().prototype(),
        Some(prototype)
    );
    assert!(
        matches!(&data.payload, ObjectPayload::Primitive(crate::engine::heap::PrimitiveObjectData::String(id)) if *id == string)
    );
    assert_eq!(state.heap.strong_count(RawId::String(string)), Ok(1));
    // Each published shape owns one prototype edge. String boxing can publish
    // both the empty shape and its length successor, retaining the former in
    // the existing shape cache. No temporary preparation edge may remain.
    assert_eq!(
        state.heap.object_strong_count(prototype),
        Ok(prototype_count + prototype_shapes(&state) - initial_shapes)
    );
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(wrapper))
        .unwrap();
    assert_eq!(
        state.heap.object_strong_count(prototype),
        Ok(prototype_count + prototype_shapes(&state) - initial_shapes)
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
        .unwrap();
    assert!(state.heap.string(string).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn to_object_public_bad_realm_and_max_prototype_precede_actual_deferred_admission() {
    for bad_realm in [true, false] {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let realm = context.realm;
        let prototype = runtime
            .0
            .state
            .borrow()
            .primitive_prototype_id_for_realm(realm, PrimitiveKind::String)
            .unwrap();
        let saved = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(prototype)
            .unwrap();
        if bad_realm {
            drop(context);
            runtime.run_gc().unwrap();
            assert!(runtime.0.state.borrow().heap.context(realm).is_err());
        } else {
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
        }
        let (input, id) = string(&runtime);
        let later = queue_rejected_admission(&runtime);
        let error = runtime.native_to_object_jsvalue(realm, input);
        if !bad_realm {
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(prototype), saved);
            assert!(matches!(
                error,
                Err(RuntimeError::Heap(HeapError::Overflow { .. }))
            ));
        } else {
            assert!(matches!(error, Err(RuntimeError::Heap(_))));
        }
        assert!(runtime.0.state.borrow().heap.string(id).is_err());
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(later),
            Ok(1)
        );
        assert!(runtime.0.deferred_references.has_pending());
        assert!(!runtime.is_poisoned());
        // Only the later real public admission visits the rejected FIFO item.
        assert!(runtime.operation().is_err());
        assert!(runtime.is_poisoned());
    }
}

#[test]
fn to_object_public_admission_failure_quarantines_prepared_prototype_and_input_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let (input, id) = string(&runtime);
    let prototype = runtime
        .0
        .state
        .borrow()
        .primitive_prototype_id_for_realm(context.realm, PrimitiveKind::String)
        .unwrap();
    let saved = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(prototype)
        .unwrap();
    let later = queue_rejected_admission(&runtime);
    assert!(matches!(
        runtime.native_to_object_jsvalue(context.realm, input),
        Err(RuntimeError::Heap(_))
    ));
    assert!(runtime.is_poisoned());
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.object_strong_count(prototype), Ok(saved + 1));
    assert_eq!(state.heap.strong_count(RawId::String(id)), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(1));
    assert!(runtime.0.deferred_references.has_pending());
}

#[test]
fn to_object_box_publication_failure_stops_before_checked_prototype_retirement() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let (input, id) = string(&runtime);
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = state
        .primitive_prototype_id_for_realm(context.realm, PrimitiveKind::String)
        .unwrap();
    let saved = state.heap.object_strong_count(prototype).unwrap();
    let before = state.heap.counts().object_nodes;
    state
        .heap
        .queue_release_for_test(RawId::Object(first))
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(later))
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(first), 1);
    let result = state.native_to_object_jsvalue(&runtime.0.poisoned, context.realm, input);
    assert!(matches!(result, Err(RuntimeError::Poisoned)));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.counts().object_nodes, before + 1);
    // One layout edge plus the unreleased checked preparation edge remain.
    assert_eq!(state.heap.object_strong_count(prototype), Ok(saved + 2));
    assert_eq!(state.heap.strong_count(RawId::String(id)), Ok(2));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}
