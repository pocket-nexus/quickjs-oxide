//! Standalone raw scopes and emitted iterator waits retain explicit ownership.
use super::*;
use crate::engine::api::{Context, Value};
use crate::engine::vm::call::{NativeInvocation, NativeInvokeMode};

fn call_with_input(
    runtime: &Runtime,
    context: &mut Context,
) -> (
    super::super::call::PreparedNativeCall,
    crate::engine::heap::ObjectId,
    crate::engine::heap::ContextId,
) {
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
    let alias = runtime.dup_jsvalue(&JsValue::Object(marker)).unwrap();
    let call = runtime
        .prepare_native_invocation_jsvalue(
            &callable,
            realm,
            target,
            min_readable_args,
            NativeInvocation::Call {
                this_value: JsValue::Object(marker),
            },
            vec![alias],
            NativeInvokeMode::Ordinary,
        )
        .unwrap()
        .into_inner();
    (call, marker, realm)
}

fn query_with_input(
    runtime: &Runtime,
    context: &mut Context,
) -> (Query, crate::engine::heap::ObjectId) {
    let (call, marker, realm) = call_with_input(runtime, context);
    let mut storage = QueryStorage::default();
    let mut query = storage.acquire(realm, Vec::new(), Finish::Root);
    native::install_waiting(runtime, &mut query, call, Resume::Identity).unwrap();
    (query, marker)
}

#[test]
fn native_query_registration_failure_retires_guarded_inputs_and_descriptor() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (call, marker, realm) = call_with_input(&runtime, &mut context);
    let mut storage = QueryStorage::default();
    let mut query = storage.acquire(realm, Vec::new(), Finish::Root);
    let saved = runtime.0.raw_execution_owners.get();
    runtime.0.raw_execution_owners.set(usize::MAX);
    let result = native::install_waiting(&runtime, &mut query, call, Resume::Identity);
    assert_eq!(runtime.0.raw_execution_owners.get(), usize::MAX);
    runtime.0.raw_execution_owners.set(saved);
    assert_eq!(
        result.unwrap_err().message(),
        "runtime invariant failed: raw execution owner count exhausted"
    );
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(query.natives.is_empty());
    assert!(!runtime.is_poisoned());
    drop(query);
    assert_eq!(runtime.0.raw_execution_owners.get(), saved);
}

#[test]
fn nested_native_scopes_share_one_query_registration_and_abandon_lifo() {
    let runtime = Runtime::new();
    let mut outer = runtime.new_context().unwrap();
    let mut inner = runtime.new_context().unwrap();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let (mut query, first) = query_with_input(&runtime, &mut outer);
    let (call, second, _) = call_with_input(&runtime, &mut inner);
    native::install_waiting(&runtime, &mut query, call, Resume::Identity).unwrap();
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    assert_eq!(runtime.0.raw_execution_owners.get(), 1);
    assert_eq!(runtime.0.state.borrow().active_frames.len(), 2);
    drop(query);
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(first).is_err());
    assert!(state.heap.object(second).is_err());
    assert!(state.active_frames.is_empty());
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn standalone_native_query_abandonment_retires_raw_owners_without_runtime_roots() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let (query, marker) = query_with_input(&runtime, &mut context);
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(marker),
        Ok(2)
    );
    assert_eq!(runtime.0.active_frame_depth.get(), 1);
    drop(query);
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
}

#[test]
fn standalone_native_query_can_outlive_runtime_without_extending_runtime_lifetime() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (query, _) = query_with_input(&runtime, &mut context);
    let weak = std::rc::Rc::downgrade(&runtime.0);
    drop(context);
    drop(runtime);
    assert!(weak.upgrade().is_none());
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(query))).is_ok());
}

#[test]
fn actual_array_next_wait_is_released_when_query_identity_installation_fails() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let parent = runtime
        .callable_from_value(context.eval("(function parent(){return 0;})").unwrap())
        .unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&parent).unwrap()
    else {
        unreachable!()
    };
    let entry = super::super::root_call::prepare_call(
        &runtime,
        context.realm,
        &parent,
        JsValue::Undefined,
        JsValue::Undefined,
        Vec::new(),
        bytecode,
        closure_slots,
    )
    .unwrap();
    let Value::Object(iterator) = context.eval("(()=>{let a=[];Object.defineProperty(a,'0',{get(){throw 99;}});a.length=1;return a.values();})()").unwrap() else { unreachable!() };
    let iterator_id = iterator.object_id();
    let source = match runtime
        .0
        .state
        .borrow()
        .heap
        .object(iterator_id)
        .unwrap()
        .payload
    {
        crate::engine::heap::ObjectPayload::ArrayIterator {
            object: Some(source),
            ..
        } => source,
        _ => unreachable!(),
    };
    let method = context
        .get_property(&iterator, &runtime.intern_property_key("next").unwrap())
        .unwrap();
    let method = runtime.callable_from_value(method).unwrap();
    let CallableExecution::Native {
        realm,
        min_readable_args,
        ..
    } = runtime.bytecode_for_callable(&method).unwrap()
    else {
        unreachable!()
    };
    let mut execution = RunningExecution::new(
        &runtime,
        super::super::execution::ExecutionLimits::default(),
    )
    .unwrap();
    let frame = push_frame(&runtime, &mut execution, entry).unwrap();
    execution
        .frames
        .current_mut(frame)
        .unwrap()
        .property_generation = u64::MAX;
    let result = start_array_next_without_pending(
        &runtime,
        &mut execution,
        frame,
        0,
        method,
        realm,
        min_readable_args,
        JsValue::Object(iterator.into_handle()),
    );
    assert!(
        result.is_err(),
        "identity exhaustion remains an invariant error"
    );
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(iterator_id).is_err());
    assert!(
        state.heap.object(source).is_err(),
        "emitted getter wait no longer retains the array"
    );
    assert_eq!(state.active_frames.len(), 1, "only caller remains");
    assert!(!runtime.is_poisoned());
    drop(state);
    drop(execution);
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
}

#[test]
fn native_query_drop_during_external_state_borrow_defers_restore_and_owned_releases() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (query, marker) = query_with_input(&runtime, &mut context);
    let state = runtime.0.state.borrow_mut();
    drop(query);
    assert_eq!(state.heap.object_strong_count(marker), Ok(2));
    assert_eq!(state.active_frames.len(), 1);
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert!(runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
    drop(state);
    drop(runtime.operation().unwrap());
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.0.deferred_references.has_pending());
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
}

#[test]
fn published_native_body_error_waits_for_raw_parent_retirement_before_returning() {
    use crate::engine::{
        api::RuntimeError,
        builtins::{continuation::NativeOperation, native::NativeFunctionId},
        heap::{GcPolicy, PropertySlot, RawId, RawValue},
        value::conversion::primitive::{PrimitiveResume, PrimitiveStep},
        vm::ToPrimitiveHint,
    };

    for malformed in [false, true] {
        let runtime = Runtime::new();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let mut context = runtime.new_context().unwrap();
        let prototype = context.function_prototype().unwrap();
        let probe = runtime
            .new_bound_native_function(
                &prototype,
                context.realm,
                NativeFunctionId::ActiveFrameProbe,
                1,
            )
            .unwrap();
        // The existing probe rejects Bool(true) without poisoning the Runtime.
        // Its activation owns no edge to the parent retired below.
        let call = runtime
            .prepare_native_invocation_jsvalue(
                &probe,
                context.realm,
                NativeFunctionId::ActiveFrameProbe,
                1,
                NativeInvocation::Call {
                    this_value: JsValue::Undefined,
                },
                vec![JsValue::Bool(true)],
                NativeInvokeMode::Ordinary,
            )
            .unwrap()
            .into_inner();
        let Value::Object(parent) = context.eval("({bad:{},suffix:{}})").unwrap() else {
            panic!("raw parent fixture")
        };
        let parent = parent.into_handle();
        let mut execution = RunningExecution::new(
            &runtime,
            super::super::execution::ExecutionLimits::default(),
        )
        .unwrap();
        let mut storage = QueryStorage::default();
        let mut query = storage.acquire(context.realm, Vec::new(), Finish::Root);
        let (resume, bad, suffix) = {
            let mut state = runtime.0.state.borrow_mut();
            let PrimitiveStep::Get { mut resume } = PrimitiveResume::start_in_state(
                &mut state,
                &runtime.0.poisoned,
                context.realm,
                JsValue::Object(parent),
                ToPrimitiveHint::Number,
            )
            .unwrap() else {
                panic!("raw primitive parent")
            };
            let (requested, _) = resume.take_get_in_state();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(requested))
                .unwrap();
            assert_eq!(state.heap.object_strong_count(parent), Ok(1));
            let [
                PropertySlot::Data(RawValue::Object(bad)),
                PropertySlot::Data(RawValue::Object(suffix)),
            ] = state.heap.object(parent).unwrap().slots.as_slice()
            else {
                panic!("ordered parent edges")
            };
            let (bad, suffix) = (*bad, *suffix);
            assert_eq!(state.heap.object_strong_count(bad), Ok(1));
            assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
            if malformed {
                // Only the final parent release reaches this broken edge.
                // The next stored child proves cleanup stops at that failure.
                state.heap.set_strong_count_for_test(RawId::Object(bad), 0);
            }
            (resume, bad, suffix)
        };
        let mut pending =
            Step::PreparedNativeBoundary(Some(Box::new(super::request::PreparedNativeBoundary {
                call,
                kind: NativeOperation::ActiveFrameProbe,
                resume: Resume::Primitive(resume),
            })));
        let error =
            native::start_published_boundary(&runtime, &mut execution, &mut query, &mut pending)
                .unwrap_err();
        assert!(matches!(pending, Step::PreparedNativeBoundary(None)));
        assert!(query.natives.is_empty());
        let state = runtime.0.state.borrow();
        assert!(
            state.active_frames.is_empty(),
            "native activation retired first"
        );
        assert!(state.heap.object(parent).is_err());
        if malformed {
            assert_eq!(error.message(), RuntimeError::Poisoned.to_string());
            assert!(runtime.is_poisoned());
            assert_eq!(state.heap.object_strong_count(bad), Ok(0));
            assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
        } else {
            assert_eq!(
                error.message(),
                "runtime invariant failed: active frame probe engine error"
            );
            assert!(!runtime.is_poisoned());
            assert!(state.heap.object(bad).is_err());
            assert!(state.heap.object(suffix).is_err());
            drop(state);
            drop(pending);
            drop(query);
            drop(execution);
            assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
        }
    }
}
