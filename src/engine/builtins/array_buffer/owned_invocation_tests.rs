//! Original receiver/newTarget owners at the legacy binary-buffer boundary.
use crate::engine::{
    api::{Context, Runtime, RuntimeError},
    builtins::native::{
        ArrayBufferNativeKind, DataViewNativeKind, NativeFunctionId, SharedArrayBufferNativeKind,
    },
    heap::{ContextId, ObjectId, RawId},
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
        panic!("native binary-buffer fixture");
    };
    (callable, realm, target, min_readable_args)
}

fn object_input(runtime: &Runtime, context: &mut Context, source: &str) -> ObjectId {
    let JsValue::Object(id) = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap() else {
        panic!("object input");
    };
    id
}

fn completion(outcome: NativeInvokeOutcome) -> Completion {
    let NativeInvokeOutcome::Completion(completion) = outcome else {
        panic!("ordinary completion");
    };
    completion
}

fn assert_finished(runtime: &Runtime) {
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn owned_binary_buffer_methods_retire_receivers_after_return_and_brand_throw() {
    for (source, receiver_source, arguments) in [
        (
            "ArrayBuffer.prototype.resize",
            "new ArrayBuffer(4,{maxByteLength:8})",
            &[0][..],
        ),
        (
            "ArrayBuffer.prototype.slice",
            "new ArrayBuffer(4)",
            &[0][..],
        ),
        (
            "ArrayBuffer.prototype.transfer",
            "new ArrayBuffer(4)",
            &[0][..],
        ),
        (
            "ArrayBuffer.prototype.transferToFixedLength",
            "new ArrayBuffer(4)",
            &[0][..],
        ),
        (
            "SharedArrayBuffer.prototype.grow",
            "new SharedArrayBuffer(4,{maxByteLength:8})",
            &[4][..],
        ),
        (
            "SharedArrayBuffer.prototype.slice",
            "new SharedArrayBuffer(4)",
            &[0][..],
        ),
        (
            "DataView.prototype.getUint8",
            "new DataView(new ArrayBuffer(4))",
            &[0][..],
        ),
        (
            "DataView.prototype.setUint8",
            "new DataView(new ArrayBuffer(4))",
            &[0, 7][..],
        ),
    ] {
        for throwing in [false, true] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, source);
            let receiver = object_input(
                &runtime,
                &mut context,
                if throwing { "({})" } else { receiver_source },
            );
            let result = completion(
                runtime
                    .invoke_native_function_jsvalue(
                        &callable,
                        realm,
                        target,
                        minimum,
                        NativeInvocation::Call {
                            this_value: JsValue::Object(receiver),
                        },
                        arguments.iter().copied().map(JsValue::Int).collect(),
                        NativeInvokeMode::Ordinary,
                    )
                    .unwrap(),
            );
            match result {
                Completion::Throw(value) if throwing => runtime.release_jsvalue(value).unwrap(),
                Completion::Return(value) if !throwing => runtime.release_jsvalue(value).unwrap(),
                _ => panic!("unexpected completion for {source}"),
            }
            assert!(
                runtime.0.state.borrow().heap.object(receiver).is_err(),
                "{source}"
            );
            assert_finished(&runtime);
        }
    }
}

#[test]
fn owned_binary_buffer_species_keeps_one_independent_return_owner() {
    for source in [
        "Object.getOwnPropertyDescriptor(ArrayBuffer,Symbol.species).get",
        "Object.getOwnPropertyDescriptor(SharedArrayBuffer,Symbol.species).get",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, source);
        let receiver = runtime.new_object(None).unwrap().into_execution_handle();
        let result = completion(
            runtime
                .invoke_native_function_jsvalue(
                    &callable,
                    realm,
                    target,
                    minimum,
                    NativeInvocation::Call {
                        this_value: JsValue::Object(receiver),
                    },
                    Vec::new(),
                    NativeInvokeMode::Ordinary,
                )
                .unwrap(),
        );
        let Completion::Return(JsValue::Object(returned)) = result else {
            panic!("species owner");
        };
        assert_eq!(returned, receiver);
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(receiver),
            Ok(1)
        );
        runtime.release_jsvalue(JsValue::Object(returned)).unwrap();
        assert!(runtime.0.state.borrow().heap.object(receiver).is_err());
        assert_finished(&runtime);
    }
}

#[test]
fn owned_data_view_buffer_getter_retires_view_but_preserves_backing_return() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) = native_fixture(
        &runtime,
        &mut context,
        "Object.getOwnPropertyDescriptor(DataView.prototype,'buffer').get",
    );
    let view = object_input(&runtime, &mut context, "new DataView(new ArrayBuffer(4))");
    let result = completion(
        runtime
            .invoke_native_function_jsvalue(
                &callable,
                realm,
                target,
                minimum,
                NativeInvocation::Call {
                    this_value: JsValue::Object(view),
                },
                Vec::new(),
                NativeInvokeMode::Ordinary,
            )
            .unwrap(),
    );
    let Completion::Return(JsValue::Object(buffer)) = result else {
        panic!("backing return owner");
    };
    assert!(runtime.0.state.borrow().heap.object(view).is_err());
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(buffer),
        Ok(1)
    );
    runtime.release_jsvalue(JsValue::Object(buffer)).unwrap();
    assert!(runtime.0.state.borrow().heap.object(buffer).is_err());
    assert_finished(&runtime);
}

#[test]
fn owned_binary_buffer_constructors_retire_original_new_target() {
    for source in ["ArrayBuffer", "SharedArrayBuffer", "DataView"] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, source);
        let function = callable.as_object().object_id();
        let original_count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(function)
            .unwrap();
        let arguments = if source == "DataView" {
            vec![JsValue::Object(object_input(
                &runtime,
                &mut context,
                "new ArrayBuffer(4)",
            ))]
        } else {
            vec![JsValue::Int(4)]
        };
        let new_target = runtime.dup_jsvalue(&JsValue::Object(function)).unwrap();
        let result = completion(
            runtime
                .invoke_native_function_jsvalue(
                    &callable,
                    realm,
                    target,
                    minimum,
                    NativeInvocation::Construct { new_target },
                    arguments,
                    NativeInvokeMode::Ordinary,
                )
                .unwrap(),
        );
        let Completion::Return(value @ JsValue::Object(_)) = result else {
            panic!("constructed {source}");
        };
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(function),
            Ok(original_count),
            "{source}"
        );
        runtime.release_jsvalue(value).unwrap();
        assert_finished(&runtime);
    }
}

#[test]
fn owned_binary_buffer_constructors_retire_new_target_after_prototype_throw() {
    for source in ["ArrayBuffer", "SharedArrayBuffer", "DataView"] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context, source);
        let proxy_source = format!(
            "new Proxy({source}, {{get(t,k,r){{if(k==='prototype')throw 7;return Reflect.get(t,k,r)}}}})"
        );
        let new_target = object_input(&runtime, &mut context, &proxy_source);
        let arguments = if source == "DataView" {
            vec![JsValue::Object(object_input(
                &runtime,
                &mut context,
                "new ArrayBuffer(4)",
            ))]
        } else {
            vec![JsValue::Int(4)]
        };
        let result = completion(
            runtime
                .invoke_native_function_jsvalue(
                    &callable,
                    realm,
                    target,
                    minimum,
                    NativeInvocation::Construct {
                        new_target: JsValue::Object(new_target),
                    },
                    arguments,
                    NativeInvokeMode::Ordinary,
                )
                .unwrap(),
        );
        assert!(
            matches!(result, Completion::Throw(JsValue::Int(7))),
            "{source}"
        );
        assert!(
            runtime.0.state.borrow().heap.object(new_target).is_err(),
            "{source}"
        );
        assert_finished(&runtime);
    }
}

#[test]
fn owned_binary_buffer_handler_error_keeps_priority_and_retires_input() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (_, realm, _, _) = native_fixture(&runtime, &mut context, "ArrayBuffer");
    let arguments = NativeArguments {
        actual_arg_count: 0,
        readable: Vec::new(),
    };
    for family in 0..3 {
        let input = runtime.new_object(None).unwrap().into_execution_handle();
        let invocation = NativeInvocation::Getter {
            this_value: JsValue::Object(input),
        };
        let result = match family {
            0 => runtime.call_array_buffer_native(
                realm,
                ArrayBufferNativeKind::Constructor,
                invocation,
                &arguments,
            ),
            1 => runtime.call_shared_array_buffer_native(
                realm,
                SharedArrayBufferNativeKind::Constructor,
                invocation,
                &arguments,
            ),
            _ => runtime.call_data_view_native(
                realm,
                DataViewNativeKind::Constructor,
                invocation,
                &arguments,
            ),
        };
        let expected = if family == 2 {
            "DataView constructor did not receive a constructor invocation"
        } else {
            "ArrayBuffer constructor did not receive a constructor invocation"
        };
        assert!(matches!(result, Err(RuntimeError::Invariant(message)) if message == expected));
        assert!(runtime.0.state.borrow().heap.object(input).is_err());
        assert_finished(&runtime);
    }
}

#[test]
fn owned_binary_buffer_release_failure_quarantines_before_later_native_owners() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) =
        native_fixture(&runtime, &mut context, "ArrayBuffer.isView");
    let function = callable.as_object().object_id();
    let original_count = runtime
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
            vec![JsValue::Object(later_argument)],
            NativeInvokeMode::Ordinary,
        )
    }));
    match result {
        Err(_) => assert!(cfg!(debug_assertions)),
        Ok(Err(RuntimeError::Poisoned)) => {}
        _ => panic!("invalid original receiver retirement did not quarantine"),
    }
    assert!(runtime.is_poisoned());
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.object_strong_count(later_argument), Ok(1));
    assert_eq!(
        state.heap.object_strong_count(function),
        Ok(original_count + 1)
    );
    assert_eq!(state.active_frames.len(), 1);
    assert_eq!(runtime.0.active_frame_depth.get(), 1);
}
