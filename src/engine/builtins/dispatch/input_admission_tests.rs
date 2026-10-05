//! The canonical call boundary owns inputs before checking its callee edge.
use super::*;
use crate::engine::{
    api::Context,
    heap::{HeapError, RawId},
};

fn callable(runtime: &Runtime, context: &mut Context) -> CallableRef {
    runtime
        .callable_from_value(context.eval("(function rejected(){throw 99})").unwrap())
        .unwrap()
}

#[test]
fn checked_callee_rejection_retires_receiver_and_aliased_arguments() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let callable = callable(&runtime, &mut context);
    let function = callable.as_object().object_id();
    let receiver = runtime.new_object(None).unwrap().into_execution_handle();
    let alias = runtime.dup_jsvalue(&JsValue::Object(receiver)).unwrap();
    let suffix = runtime.new_object(None).unwrap().into_execution_handle();
    let before = {
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.object_strong_count(function).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(function), u32::MAX);
        before
    };
    let result = runtime.call_internal_jsvalue(
        context.realm,
        &callable,
        JsValue::Object(receiver),
        vec![alias, JsValue::Object(suffix)],
    );
    let mut state = runtime.0.state.borrow_mut();
    let rejected_count = state.heap.object_strong_count(function).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(function), before);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(rejected_count, u32::MAX);
    assert!(state.heap.object(receiver).is_err());
    assert!(state.heap.object(suffix).is_err());
    assert!(state.active_frames.is_empty());
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn checked_callee_borrow_rejection_coordinates_owned_inputs_without_poison() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let callable = callable(&runtime, &mut context);
    let receiver = runtime.new_object(None).unwrap().into_execution_handle();
    let argument = runtime.new_object(None).unwrap().into_execution_handle();
    let state = runtime.0.state.borrow_mut();
    let result = runtime.call_internal_jsvalue(
        context.realm,
        &callable,
        JsValue::Object(receiver),
        vec![JsValue::Object(argument)],
    );
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Invariant(_)))
    ));
    assert_eq!(state.heap.object_strong_count(receiver), Ok(1));
    assert_eq!(state.heap.object_strong_count(argument), Ok(1));
    assert!(runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
    drop(state);
    runtime.drain_deferred_references().unwrap();
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(receiver).is_err());
    assert!(state.heap.object(argument).is_err());
    assert!(state.active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn checked_callee_rejection_stops_after_destructive_receiver_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let callable = callable(&runtime, &mut context);
    let function = callable.as_object().object_id();
    let receiver = runtime.new_object(None).unwrap().into_execution_handle();
    let argument = runtime.new_object(None).unwrap().into_execution_handle();
    let first = runtime.new_object(None).unwrap().into_execution_handle();
    let later = runtime.new_object(None).unwrap().into_execution_handle();
    let before = {
        let mut state = runtime.0.state.borrow_mut();
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
        let before = state.heap.object_strong_count(function).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(function), u32::MAX);
        before
    };
    let result = runtime.call_internal_jsvalue(
        context.realm,
        &callable,
        JsValue::Object(receiver),
        vec![JsValue::Object(argument)],
    );
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(function), before);
    assert!(matches!(result, Err(RuntimeError::Poisoned)));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(argument), Ok(1));
    assert_eq!(state.heap.object_strong_count(first), Ok(1));
    assert_eq!(state.heap.strong_count(RawId::Object(later)), Ok(0));
    assert!(state.active_frames.is_empty());
}
