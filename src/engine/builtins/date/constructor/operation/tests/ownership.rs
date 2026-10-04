//! Genuine producer checks and fatal retirement versus removed traversal roots.
use super::*;
use crate::engine::heap::ObjectPayload;

#[test]
fn date_function_realm_borrows_saturated_initial_bound_and_proxy_targets() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let original = object(
        &mut context,
        "var innerRealmTarget = function RealmTarget() {}; var innerRealmBound = innerRealmTarget.bind(null); new Proxy(innerRealmBound, {})",
    );
    let callable = runtime.as_callable(&original).unwrap().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let mut cursor = original.object_id();
    let mut saved = Vec::new();
    loop {
        saved.push((cursor, state.heap.object_strong_count(cursor).unwrap()));
        cursor = match &state.heap.object(cursor).unwrap().payload {
            ObjectPayload::Proxy(data) => data.target,
            ObjectPayload::BoundFunction { target, .. } => *target,
            _ => break,
        };
    }
    for (id, _) in &saved {
        state
            .heap
            .set_strong_count_for_test(RawId::Object(*id), u32::MAX);
    }
    assert!(
        matches!(state.function_realm_from_jsvalue(&runtime.0.poisoned, context.realm, &JsValue::Object(original.object_id())).unwrap(), FunctionRealmOutcome::Value(realm) if realm == context.realm)
    );
    drop(state);
    assert!(
        matches!(runtime.function_realm(context.realm, &callable).unwrap(), NativeConversion::Value(realm) if realm == context.realm)
    );
    assert!(
        matches!(runtime.function_realm_from_jsvalue(context.realm, &JsValue::Object(original.object_id())).unwrap(), NativeConversion::Value(realm) if realm == context.realm)
    );
    let mut state = runtime.0.state.borrow_mut();
    for (id, count) in saved {
        state
            .heap
            .set_strong_count_for_test(RawId::Object(id), count);
    }
    assert!(!runtime.is_poisoned());
}
#[test]
fn date_function_realm_revocation_still_produces_owned_caller_error() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let proxy = object(
        &mut context,
        "var realmRevoked = Proxy.revocable(function(){}, {}); realmRevoked.revoke(); realmRevoked.proxy",
    );
    let mut state = runtime.0.state.borrow_mut();
    let count = state.heap.object_strong_count(proxy.object_id()).unwrap();
    let result = state
        .function_realm_from_jsvalue(
            &runtime.0.poisoned,
            context.realm,
            &JsValue::Object(proxy.object_id()),
        )
        .unwrap();
    let FunctionRealmOutcome::CyclePublishedThrow(value) = result else {
        panic!("fresh revoked Error")
    };
    assert_eq!(
        state.heap.object_strong_count(proxy.object_id()).unwrap(),
        count
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
}
#[test]
fn date_constructor_real_newtarget_and_fallback_prototype_retains_still_reject_max() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let saved = state.heap.object_strong_count(target.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(target.object_id()), u32::MAX);
    assert!(matches!(
        start(
            &mut state,
            &runtime,
            context.realm,
            DateNativeKind::Constructor,
            JsValue::Object(target.object_id()),
            vec![]
        ),
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    state
        .heap
        .set_strong_count_for_test(RawId::Object(target.object_id()), saved);
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Object(target.object_id()),
        vec![],
    )
    .unwrap();
    let DateConstructorStep::Read {
        receiver, resume, ..
    } = step
    else {
        panic!("prototype read")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, receiver)
        .unwrap();
    let proto = state
        .heap
        .context(context.realm)
        .unwrap()
        .date_prototype
        .unwrap();
    let proto_count = state.heap.object_strong_count(proto).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(proto), u32::MAX);
    let result = resume.resume_in_state(
        &mut state,
        &runtime.0.poisoned,
        Completion::Return(JsValue::Null),
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(proto), proto_count);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(
        state.heap.object_strong_count(target.object_id()).unwrap(),
        saved
    );
    assert!(!runtime.is_poisoned());
}
#[test]
fn date_constructor_alias_snapshot_retirement_preserves_original_newtarget_and_argv() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let alias = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let count = state.heap.object_strong_count(alias.object_id()).unwrap();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Object(alias.object_id()),
        vec![
            JsValue::Object(alias.object_id()),
            JsValue::Object(alias.object_id()),
        ],
    )
    .unwrap();
    assert_eq!(
        state.heap.object_strong_count(alias.object_id()).unwrap(),
        count + 3
    );
    retired(step, &mut state, &runtime);
    assert_eq!(
        state.heap.object_strong_count(alias.object_id()).unwrap(),
        count
    );
}
fn queue_invalid(
    state: &mut RuntimeState,
    first: crate::engine::heap::ObjectId,
    later: crate::engine::heap::ObjectId,
) {
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
}
#[test]
fn date_constructor_publication_failure_quarantines_target_prototype_and_queue_suffix() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap().into_handle();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let proto = state
        .heap
        .context(context.realm)
        .unwrap()
        .date_prototype
        .unwrap();
    let warm = state
        .new_date_object(&runtime.0.poisoned, proto, 0.0)
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(warm))
        .unwrap();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Object(target),
        vec![],
    )
    .unwrap();
    let DateConstructorStep::Read {
        receiver, resume, ..
    } = step
    else {
        panic!("prototype read")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, receiver)
        .unwrap();
    let prototype = state.dup_jsvalue(&JsValue::Object(proto)).unwrap();
    let target_count = state.heap.object_strong_count(target).unwrap();
    let nodes = state.heap.counts().object_nodes;
    queue_invalid(&mut state, first, later);
    let result = resume.resume_in_state(
        &mut state,
        &runtime.0.poisoned,
        Completion::Return(prototype),
    );
    assert!(matches!(result, Err(RuntimeError::Poisoned)));
    assert_eq!(state.heap.counts().object_nodes, nodes + 1);
    assert_eq!(state.heap.object_strong_count(target), Ok(target_count));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}
#[test]
fn date_constructor_terminal_first_fatal_retires_no_suffix_and_keeps_throw_armed() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap().into_handle();
    let first = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let thrown = runtime.new_object(None).unwrap().into_handle();
    let doomed = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let resume = DateConstructorResume(Box::new(DateConstructorResumeState {
        converted: JsValue::Undefined,
        prototype: None,
        realm: context.realm,
        kind: DateNativeKind::Parse,
        new_target: JsValue::Object(target),
        arguments: std::collections::VecDeque::from([
            JsValue::Object(first),
            JsValue::Object(suffix),
        ]),
        fields: DEFAULT_DATE_FIELDS,
        index: 0,
        value: f64::NAN,
        phase: Phase::Parse,
    }));
    let mut state = runtime.0.state.borrow_mut();
    queue_invalid(&mut state, doomed, later);
    let result = resume.string_in_state(
        &mut state,
        &runtime.0.poisoned,
        runtime.0.host_services.as_ref(),
        NativeConversion::Throw(JsValue::Object(thrown)),
    );
    assert!(matches!(result, Err(RuntimeError::Poisoned)));
    assert_eq!(state.heap.object_strong_count(first), Ok(0));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(target), Ok(1));
    assert_eq!(state.heap.object_strong_count(thrown), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
}
