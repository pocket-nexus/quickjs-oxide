//! Borrowed preparation and the explicit standalone lifetime adapter.
use super::*;
use crate::engine::{api::Context, vm::call::CallableExecution};
use std::rc::Rc;

fn native_fixture(
    runtime: &Runtime,
    context: &mut Context,
) -> (CallableRef, ContextId, NativeFunctionId, u8) {
    let callable = runtime
        .callable_from_value(context.eval("Number.isFinite").unwrap())
        .unwrap();
    let CallableExecution::Native {
        target,
        realm,
        min_readable_args,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("native fixture");
    };
    (callable, realm, target, min_readable_args)
}

#[test]
fn borrowed_internal_preparation_keeps_aliases_without_runtime_owner() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context);
    let function = callable.as_object().object_id();
    let original_function_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(function)
        .unwrap();
    let marker = runtime.new_object(None).unwrap().into_execution_handle();
    let alias = runtime.dup_jsvalue(&JsValue::Object(marker)).unwrap();
    let owners = Rc::strong_count(&runtime.0);
    let weak_owners = Rc::weak_count(&runtime.0);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let call = runtime
        .prepare_native_invocation_jsvalue(
            &callable,
            realm,
            target,
            minimum,
            NativeInvocation::Call { this_value: alias },
            vec![JsValue::Object(marker)],
            NativeInvokeMode::Ordinary,
        )
        .unwrap();
    assert_eq!(Rc::strong_count(&runtime.0), owners);
    assert_eq!(Rc::weak_count(&runtime.0), weak_owners);
    assert_eq!(call.activation.arguments.actual_arg_count, 1);
    assert_eq!(
        call.activation.arguments.readable[0],
        JsValue::Object(marker)
    );
    {
        let state = runtime.0.state.borrow();
        assert_eq!(state.heap.object_strong_count(marker), Ok(2));
        assert_eq!(
            state.heap.object_strong_count(function),
            Ok(original_function_count + 1)
        );
        assert_eq!(state.active_frames.len(), 1);
    }
    #[cfg(feature = "profiling")]
    {
        let events = profile.snapshot().owned_execution_events;
        // Public diagnostic publication and the checked callable promotion
        // still use their existing transient roots. Preparation adds no owner.
        assert_eq!(events.get("runtime.clone"), Some(&2));
    }
    drop(callable);
    drop(context);
    assert_eq!(Rc::strong_count(&runtime.0), 1);
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().heap.object(marker).is_ok());
    drop(call);
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(marker).is_err());
    assert!(state.active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn borrowed_public_preparation_outlives_input_roots_without_owning_runtime() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context);
    let marker = runtime.new_object(None).unwrap();
    let marker_id = marker.object_id();
    let arguments = vec![Value::Object(marker)];
    let owners = Rc::strong_count(&runtime.0);
    let weak_owners = Rc::weak_count(&runtime.0);
    let call = runtime
        .prepare_native_invocation(
            &callable,
            realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            &arguments,
            NativeInvokeMode::Ordinary,
        )
        .unwrap();
    assert_eq!(Rc::strong_count(&runtime.0), owners);
    assert_eq!(Rc::weak_count(&runtime.0), weak_owners);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(marker_id),
        Ok(2)
    );
    drop(arguments);
    drop(callable);
    drop(context);
    assert_eq!(Rc::strong_count(&runtime.0), 1);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(marker_id),
        Ok(1)
    );
    let result = call
        .finish(Ok(NativeInvokeOutcome::Completion(Completion::Return(
            JsValue::Int(42),
        ))))
        .unwrap();
    assert!(matches!(
        result,
        NativeInvokeOutcome::Completion(Completion::Return(JsValue::Int(42)))
    ));
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(marker_id).is_err());
    assert!(state.active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn standalone_preparation_adapter_outlives_runtime_binding() {
    let runtime = Runtime::new();
    let weak = Rc::downgrade(&runtime.0);
    let mut context = runtime.new_context().unwrap();
    let (callable, realm, target, minimum) = native_fixture(&runtime, &mut context);
    let marker = runtime.new_object(None).unwrap().into_execution_handle();
    let call = runtime
        .prepare_native_invocation_jsvalue(
            &callable,
            realm,
            target,
            minimum,
            NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            vec![JsValue::Object(marker)],
            NativeInvokeMode::Ordinary,
        )
        .unwrap();
    let owners = Rc::strong_count(&runtime.0);
    let standalone = call.into_standalone();
    assert_eq!(Rc::strong_count(&runtime.0), owners + 1);
    drop(callable);
    drop(context);
    drop(runtime);
    assert_eq!(weak.strong_count(), 1);
    assert_eq!(
        standalone.activation.arguments.readable[0],
        JsValue::Object(marker)
    );
    // This observer keeps the heap available to check retirement after dropping
    // the standalone owner; no originating Runtime binding remains.
    let inner = weak.upgrade().unwrap();
    assert_eq!(inner.state.borrow().active_frames.len(), 1);
    assert!(inner.state.borrow().heap.object(marker).is_ok());
    drop(standalone);
    assert!(inner.state.borrow().active_frames.is_empty());
    assert!(inner.state.borrow().heap.object(marker).is_err());
    assert_eq!(inner.active_frame_depth.get(), 0);
    assert!(!inner.poisoned.get());
    drop(inner);
    assert!(weak.upgrade().is_none());
}
