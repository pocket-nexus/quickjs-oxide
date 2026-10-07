//! Owned legacy Date invocation retirement at the actual Runtime boundary.
use crate::engine::{
    api::{Context, Runtime, RuntimeError},
    builtins::native::NativeFunctionId,
    heap::ContextId,
    object::CallableRef,
    value::JsValue,
    vm::{
        Completion,
        call::{
            CallableExecution, NativeArguments, NativeInvocation, NativeInvokeMode,
            NativeInvokeOutcome,
        },
    },
};

fn native_fixture(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
) -> (CallableRef, ContextId, NativeFunctionId, u8) {
    let callable = runtime
        .callable_from_value(context.eval(source).unwrap())
        .unwrap();
    let CallableExecution::Native {
        target,
        realm,
        min_readable_args,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("native Date fixture");
    };
    (callable, realm, target, min_readable_args)
}

#[test]
fn owned_legacy_date_dispatch_retires_receivers_after_return_and_throw() {
    for (source, receiver, throwing) in [
        ("Date.parse", "({})", false),
        ("Date.prototype.setTime", "new Date(0)", false),
        ("Date.prototype.setTime", "({})", true),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, source);
        let receiver = runtime
            .into_jsvalue(context.eval(receiver).unwrap())
            .unwrap();
        let JsValue::Object(receiver_id) = receiver else {
            panic!("object receiver");
        };
        let result = runtime
            .invoke_native_function_jsvalue(
                &callable,
                realm,
                target,
                minimum,
                NativeInvocation::Call {
                    this_value: receiver,
                },
                vec![JsValue::Int(0)],
                NativeInvokeMode::Ordinary,
            )
            .unwrap();
        let NativeInvokeOutcome::Completion(completion) = result else {
            panic!("ordinary Date completion");
        };
        match completion {
            Completion::Throw(value) if throwing => runtime.release_jsvalue(value).unwrap(),
            Completion::Return(value) if !throwing => runtime.release_jsvalue(value).unwrap(),
            _ => panic!("unexpected Date completion"),
        }
        let state = runtime.0.state.borrow();
        assert!(state.heap.object(receiver_id).is_err(), "{source}");
        assert!(state.active_frames.is_empty(), "{source}");
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn owned_date_entry_retires_invocation_on_handler_invariant_error() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (_callable, realm, target, _minimum) = native_fixture(&runtime, &mut context, "Date.parse");
    let NativeFunctionId::Date(kind) = target else {
        panic!("Date selector");
    };
    let marker = runtime.new_object(None).unwrap().into_execution_handle();
    let result = runtime.call_date_native(
        realm,
        kind,
        NativeInvocation::Getter {
            this_value: JsValue::Object(marker),
        },
        &NativeArguments {
            actual_arg_count: 0,
            readable: Vec::new(),
        },
    );
    assert!(matches!(
        result,
        Err(RuntimeError::Invariant(
            "Date constructor/static invocation mismatch"
        ))
    ));
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn owned_date_constructor_retires_original_new_target_after_success() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, "Date");
    let function = callable.as_object().object_id();
    let original_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(function)
        .unwrap();
    let new_target = runtime.dup_jsvalue(&JsValue::Object(function)).unwrap();
    let result = runtime
        .invoke_native_function_jsvalue(
            &callable,
            realm,
            target,
            minimum,
            NativeInvocation::Construct { new_target },
            vec![JsValue::Int(0)],
            NativeInvokeMode::Ordinary,
        )
        .unwrap();
    let NativeInvokeOutcome::Completion(Completion::Return(value @ JsValue::Object(_))) = result
    else {
        panic!("constructed Date");
    };
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(function),
        Ok(original_count)
    );
    runtime.release_jsvalue(value).unwrap();
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn owned_date_constructor_retires_new_target_after_prototype_throw() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, "Date");
    let new_target = runtime
        .into_jsvalue(context.eval("new Proxy(Date, {get(t,k,r){if(k==='prototype')throw 7;return Reflect.get(t,k,r)}})").unwrap())
        .unwrap();
    let JsValue::Object(new_target_id) = new_target else {
        panic!("object newTarget");
    };
    let result = runtime
        .invoke_native_function_jsvalue(
            &callable,
            realm,
            target,
            minimum,
            NativeInvocation::Construct { new_target },
            vec![JsValue::Int(0)],
            NativeInvokeMode::Ordinary,
        )
        .unwrap();
    assert!(matches!(
        result,
        NativeInvokeOutcome::Completion(Completion::Throw(JsValue::Int(7)))
    ));
    assert!(runtime.0.state.borrow().heap.object(new_target_id).is_err());
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "relies on the debug assertion that stops an invalid root release"
)]
fn owned_date_release_failure_quarantines_before_remaining_native_owners() {
    use crate::engine::heap::RawId;
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, "Date.parse");
    let function = callable.as_object().object_id();
    let function_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(function)
        .unwrap();
    let receiver = runtime.new_object(None).unwrap().into_execution_handle();
    let later_argument = runtime.new_object(None).unwrap().into_execution_handle();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(receiver), 0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.invoke_native_function_jsvalue(
            &callable,
            realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Object(receiver),
            },
            vec![JsValue::Int(0), JsValue::Object(later_argument)],
            NativeInvokeMode::Ordinary,
        )
    }));
    match result {
        Err(_) => assert!(cfg!(debug_assertions)),
        Ok(Err(RuntimeError::Poisoned)) => {}
        _ => panic!("invalid Date receiver retirement did not quarantine"),
    }
    assert!(runtime.is_poisoned());
    let state = runtime.0.state.borrow();
    // The Date resume retired its own temporary argument copy before the
    // original receiver failed. Quarantine blocks the activation's later owners.
    assert_eq!(state.heap.object_strong_count(later_argument), Ok(1));
    assert_eq!(
        state.heap.object_strong_count(function),
        Ok(function_count + 1)
    );
    assert_eq!(state.active_frames.len(), 1);
    assert_eq!(runtime.0.active_frame_depth.get(), 1);
}
