//! Integration witnesses for the actual single-dispatch execution consumer.
use crate::engine::{
    api::{Runtime, Value},
    value::JsValue,
    vm::{
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
    },
};

fn constructor_execution(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    argument: JsValue,
) -> (RunningExecution, crate::engine::vm::frame::FrameId) {
    let callable = runtime
        .callable_from_value(
            context
                .eval("(function invoke(C,arg){return new C(arg)})")
                .unwrap(),
        )
        .unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        unreachable!()
    };
    // Keep the empty instance layout live. A later injected cleanup failure
    // must happen after receiver publication, rather than during shape setup.
    let callee = runtime
        .into_jsvalue(
            context
                .eval("(()=>{function C(arg){return arg};C.warm=new C();return C})()")
                .unwrap(),
        )
        .unwrap();
    let entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        context.realm,
        &callable,
        JsValue::Undefined,
        JsValue::Undefined,
        vec![callee, argument],
        bytecode,
        closure_slots,
    )
    .unwrap();
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let parent = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    (execution, parent)
}

#[test]
fn ordinary_child_and_parent_execute_under_one_state_access() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let callable = runtime
        .callable_from_value(
            context
                .eval("(function invoke(fn,arg){let result=fn(arg);return result})")
                .unwrap(),
        )
        .unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        unreachable!()
    };
    let callee = runtime
        .into_jsvalue(context.eval("(function child(arg){return arg})").unwrap())
        .unwrap();
    let object = runtime.new_object(None).unwrap().into_handle();
    let entry = crate::engine::vm::root_call::prepare_call(
        &runtime,
        context.realm,
        &callable,
        JsValue::Undefined,
        JsValue::Undefined,
        vec![callee, JsValue::Object(object)],
        bytecode,
        closure_slots,
    )
    .unwrap();
    let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
    let parent = crate::engine::vm::driver::push_frame(&runtime, &mut execution, entry).unwrap();
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    {
        let mut state = runtime.0.state.borrow_mut();
        let action =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, parent).unwrap();
        assert!(matches!(action, super::VmAction::Complete));
        assert_eq!(execution.frames.current_id(), Some(parent));
        assert_eq!(execution.pending, Some(JsValue::Object(object)));
        assert!(state.heap.object_strong_count(object).unwrap() > 0);
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
        assert!(!runtime.0.deferred_references.has_pending());
    }
    #[cfg(feature = "profiling")]
    {
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
        assert_eq!(events.get("core.pc_authentication"), Some(&1));
        assert_eq!(events.get("core.internal_call"), Some(&1));
        assert_eq!(events.get("core.internal_return"), Some(&1));
    }
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(object).is_err());
    assert!(!runtime.is_poisoned());
    assert_eq!(context.eval("1+1").unwrap(), Value::Int(2));
}

#[test]
fn constructor_gc_services_published_inputs_inside_the_execution_segment() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let Value::Object(marker) = context
        .eval("(()=>{let marker={};marker.self=marker;return marker})()")
        .unwrap()
    else {
        unreachable!()
    };
    let marker = marker.into_handle();
    let Value::Object(garbage) = context
        .eval("(()=>{let garbage={};garbage.self=garbage;return garbage})()")
        .unwrap()
    else {
        unreachable!()
    };
    let garbage = garbage.into_handle();
    runtime.release_jsvalue(JsValue::Object(garbage)).unwrap();
    let (mut execution, parent) =
        constructor_execution(&runtime, &mut context, JsValue::Object(marker));
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    runtime.0.gc_pressure.remaining.set(1);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(state.heap.object(garbage).is_ok());
        let action =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, parent).unwrap();
        assert!(matches!(action, super::VmAction::Complete));
        assert_eq!(execution.frames.current_id(), Some(parent));
        assert_eq!(execution.pending, Some(JsValue::Object(marker)));
        assert!(state.heap.object(marker).is_ok());
        assert!(state.heap.object(garbage).is_err());
        assert!(runtime.0.gc_pressure.remaining.get() > 0);
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
        assert!(!runtime.0.deferred_references.has_pending());
    }
    #[cfg(feature = "profiling")]
    {
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
        assert_eq!(events.get("core.pc_authentication"), Some(&1));
        assert_eq!(events.get("core.window_authentication"), Some(&1));
        assert_eq!(events.get("slot_authentication"), Some(&1));
        assert_eq!(events.get("core.internal_construct"), Some(&1));
        assert_eq!(events.get("core.internal_return"), Some(&1));
    }
    drop(execution);
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn constructor_publication_failure_quarantines_before_execution_cleanup() {
    use crate::engine::heap::RawId;
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, parent) =
        constructor_execution(&runtime, &mut context, JsValue::Object(marker));
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let marker_count;
    {
        let mut state = runtime.0.state.borrow_mut();
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
        assert!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, parent,).is_err()
        );
        assert!(runtime.is_poisoned());
        assert_eq!(state.heap.counts().object_nodes, before + 1);
        assert_eq!(state.heap.object_strong_count(later), Ok(0));
        assert!(state.heap.has_pending_zero_cleanup());
        assert_eq!(execution.frames.current_id(), Some(parent));
        assert!(execution.pending.is_none());
        marker_count = state.heap.object_strong_count(marker).unwrap();
        assert!(marker_count > 0);
    }
    // Poisoned execution abandons its references without re-entering the
    // failed cleanup. State access has ended before the outer Drop runs.
    drop(execution);
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.object_strong_count(marker), Ok(marker_count));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(!runtime.0.deferred_references.has_pending());
}
