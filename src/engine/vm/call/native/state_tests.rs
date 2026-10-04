//! Native lifecycle witnesses under one caller-held state access.
use super::*;
use crate::engine::{
    api::{Context, Value},
    heap::RawId,
    vm::call::{CallableExecution, NativeInvocationAdaptation},
};

fn metadata(
    runtime: &Runtime,
    context: &mut Context,
    name: &str,
) -> (ObjectId, NativeFunctionId, ContextId, u8) {
    let callable = runtime
        .callable_from_value(context.eval(name).unwrap())
        .unwrap();
    let CallableExecution::Native {
        target,
        realm,
        min_readable_args,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("native callable");
    };
    (
        callable.into_object().into_execution_handle(),
        target,
        realm,
        min_readable_args,
    )
}

#[test]
fn raw_native_activation_keeps_aliases_and_padding_without_runtime_owners() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (function, target, realm, minimum) = metadata(&runtime, &mut context, "Number.isFinite");
    let marker = runtime.new_object(None).unwrap().into_handle();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let alias = state.dup_jsvalue(&JsValue::Object(marker)).unwrap();
    let call = state
        .prepare_native_call(
            &runtime.0.poisoned,
            runtime.domain_id(),
            function,
            realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Object(marker),
            },
            vec![alias],
            NativeInvokeMode::Ordinary,
            true,
            None,
        )
        .unwrap();
    assert_eq!(state.heap.object_strong_count(marker), Ok(2));
    assert_eq!(call.activation.arguments.actual_arg_count, 1);
    assert_eq!(
        call.activation.arguments.readable.len(),
        1usize.max(usize::from(minimum))
    );
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    let result = match state
        .adapt_native_invocation_borrowed(
            &runtime.0.poisoned,
            target,
            realm,
            &call.invocation,
            &call.activation.arguments,
        )
        .unwrap()
    {
        NativeInvocationAdaptation::Invoke(invocation) => {
            let result = state
                .dispatch_state_native_body(
                    &runtime.0.poisoned,
                    runtime.0.host_services.as_ref(),
                    target,
                    realm,
                    invocation.as_ref(),
                    &call.activation.arguments,
                )
                .unwrap();
            invocation
                .release_in_state(&mut state, &runtime.0.poisoned)
                .unwrap();
            result
        }
        _ => panic!("generic predicate invocation"),
    };
    let (result, empty) =
        call.finish_completion_reusing(&mut state, &runtime.0.poisoned, Ok(result));
    assert!(matches!(
        result,
        Ok(Completion::Return(JsValue::Bool(false)))
    ));
    assert!(empty.is_empty());
    assert!(state.heap.object(marker).is_err());
    assert!(state.active_frames.is_empty());
    assert!(!runtime.0.deferred_references.has_pending());
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
}

#[test]
fn raw_native_token_failure_releases_prepublication_owners_once() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (function, target, realm, minimum) = metadata(&runtime, &mut context, "Number.isInteger");
    let marker = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let expected_function = state.heap.object_strong_count(function).unwrap() - 1;
    let alias = state.dup_jsvalue(&JsValue::Object(marker)).unwrap();
    let next = state.next_active_frame_token;
    state.next_active_frame_token = u64::MAX;
    assert!(matches!(
        state.prepare_native_call(
            &runtime.0.poisoned,
            runtime.domain_id(),
            function,
            realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Object(marker)
            },
            vec![alias],
            NativeInvokeMode::Ordinary,
            true,
            None
        ),
        Err(RuntimeError::Invariant(
            "active-frame token space was exhausted"
        ))
    ));
    state.next_active_frame_token = next;
    assert!(state.heap.object(marker).is_err());
    assert_eq!(
        state.heap.object_strong_count(function),
        Ok(expected_function)
    );
    assert!(state.active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn raw_native_abandonment_stops_before_suffix_frame_and_callee_after_failure() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (function, target, realm, minimum) = metadata(&runtime, &mut context, "Number.isFinite");
    let first = runtime.new_object(None).unwrap().into_handle();
    let broken = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let callee_count = state.heap.object_strong_count(function).unwrap();
    let mut call = state
        .prepare_native_call(
            &runtime.0.poisoned,
            runtime.domain_id(),
            function,
            realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            vec![
                JsValue::Object(first),
                JsValue::Object(broken),
                JsValue::Object(suffix),
            ],
            NativeInvokeMode::Ordinary,
            true,
            None,
        )
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(broken), 0);
    call.abandon(&mut state, &runtime.0.poisoned);
    assert!(runtime.is_poisoned());
    assert!(state.heap.object(first).is_err());
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(function), Ok(callee_count));
    assert_eq!(state.active_frames.len(), 1);
}

#[test]
fn state_constructor_or_function_adaptation_keeps_original_until_finish() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (function, target, realm, minimum) = metadata(&runtime, &mut context, "Number");
    let marker = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let call = state
        .prepare_native_call(
            &runtime.0.poisoned,
            runtime.domain_id(),
            function,
            realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Object(marker),
            },
            vec![],
            NativeInvokeMode::Ordinary,
            true,
            None,
        )
        .unwrap();
    let adapted = state
        .adapt_native_invocation_borrowed(
            &runtime.0.poisoned,
            target,
            realm,
            &call.invocation,
            &call.activation.arguments,
        )
        .unwrap();
    let NativeInvocationAdaptation::Invoke(invocation) = adapted else {
        panic!("constructor sentinel");
    };
    assert!(matches!(
        invocation.as_ref(),
        NativeInvocation::Construct {
            new_target: JsValue::Undefined
        }
    ));
    assert_eq!(state.heap.object_strong_count(marker), Ok(1));
    invocation
        .release_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    let (result, _) = call.finish_completion_reusing(
        &mut state,
        &runtime.0.poisoned,
        Ok(Completion::Return(JsValue::Int(0))),
    );
    assert!(matches!(result, Ok(Completion::Return(JsValue::Int(0)))));
    assert!(state.heap.object(marker).is_err());
    assert!(state.active_frames.is_empty());
}

#[test]
fn state_iterator_result_adopts_value_and_defines_ordered_fields() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let object = {
        let mut state = runtime.0.state.borrow_mut();
        let object = state
            .new_iterator_result_jsvalue(
                &runtime.0.poisoned,
                context.realm,
                JsValue::Object(marker),
                true,
            )
            .unwrap();
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert_eq!(state.heap.object_strong_count(object), Ok(1));
        assert_eq!(state.iterator_result_allocations, 1);
        object
    };
    let object = crate::engine::object::ObjectRef::from_owned_handle(runtime.clone(), object);
    assert_eq!(
        context
            .get_property(&object, &runtime.intern_property_key("done").unwrap())
            .unwrap(),
        Value::Bool(true)
    );
    let keys = runtime.own_property_keys(&object).unwrap();
    assert_eq!(keys.len(), 2);
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object(object.object_id())
            .unwrap()
            .slots
            .len(),
        2
    );
    drop(keys);
    drop(object);
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn native_state_guard_marks_unwind_before_any_owner_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (function, target, realm, minimum) = metadata(&runtime, &mut context, "Number.isNaN");
    let marker = runtime.new_object(None).unwrap().into_handle();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut state = runtime.0.state.borrow_mut();
        let call = state
            .prepare_native_call(
                &runtime.0.poisoned,
                runtime.domain_id(),
                function,
                realm,
                target,
                minimum,
                NativeInvocation::Call {
                    this_value: JsValue::Object(marker),
                },
                vec![],
                NativeInvokeMode::Ordinary,
                true,
                None,
            )
            .unwrap();
        let _owner = NativeStateGuard::new(&mut state, &runtime.0.poisoned, call);
        panic!("resident native body failure");
    }));
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.object_strong_count(marker), Ok(1));
    assert_eq!(state.active_frames.len(), 1);
}

#[test]
fn state_iterator_result_checked_prototype_failure_consumes_only_producer() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = state.heap.context(context.realm).unwrap().object_prototype;
    let count = state.heap.object_strong_count(prototype).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
    let result = state.new_iterator_result_jsvalue(
        &runtime.0.poisoned,
        context.realm,
        JsValue::Object(marker),
        false,
    );
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(
            crate::engine::heap::HeapError::Overflow { .. }
        ))
    ));
    assert_eq!(state.heap.object_strong_count(prototype), Ok(u32::MAX));
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), count);
    assert!(state.heap.object(marker).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn migrated_body_does_not_bypass_raw_iterator_cproto_rejection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let callable = runtime
        .callable_from_value(context.eval("Number.isFinite").unwrap())
        .unwrap();
    let CallableExecution::Native {
        target,
        realm,
        min_readable_args,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        unreachable!()
    };
    let marker = runtime.new_object(None).unwrap().into_handle();
    let result = runtime.invoke_native_function_jsvalue(
        &callable,
        realm,
        target,
        min_readable_args,
        NativeInvocation::Call {
            this_value: JsValue::Object(marker),
        },
        vec![JsValue::Int(3)],
        NativeInvokeMode::IteratorNextRaw,
    );
    assert!(matches!(
        result,
        Err(RuntimeError::Invariant(
            "raw iterator-next dispatch targeted another native cproto"
        ))
    ));
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}
