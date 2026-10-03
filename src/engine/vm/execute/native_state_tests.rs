//! Actual held-state Call/TailCall consumers for the complete migrated bodies.
use crate::engine::{
    api::{Runtime, Value},
    value::JsValue,
    vm::{
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
    },
};

fn native_execution(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    name: &str,
    argument: Value,
    tail: bool,
) -> (RunningExecution, crate::engine::vm::frame::FrameId) {
    let source = if tail {
        "(function invoke(fn,arg){return fn(arg)})"
    } else {
        "(function invoke(fn,arg){let result=fn(arg);return result})"
    };
    let callable = runtime
        .callable_from_value(context.eval(source).unwrap())
        .unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("ordinary caller");
    };
    let function = runtime.into_jsvalue(context.eval(name).unwrap()).unwrap();
    let argument = runtime.into_jsvalue(argument).unwrap();
    let entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        context.realm,
        &callable,
        JsValue::Undefined,
        JsValue::Undefined,
        vec![function, argument],
        bytecode,
        closure_slots,
    )
    .unwrap();
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    (execution, id)
}

#[test]
fn six_native_selectors_finish_in_the_actual_resident_loop_without_runtime_clones() {
    for (name, input, expected) in [
        (
            "Number.isNaN",
            Value::Float(f64::NAN),
            Some(JsValue::Bool(true)),
        ),
        ("Number.isFinite", Value::Int(3), Some(JsValue::Bool(true))),
        (
            "Number.isInteger",
            Value::Float(1.5),
            Some(JsValue::Bool(false)),
        ),
        (
            "Number.isSafeInteger",
            Value::Float(9_007_199_254_740_992.0),
            Some(JsValue::Bool(false)),
        ),
        ("Math.random", Value::Undefined, None),
        (
            "Function.prototype",
            Value::Int(3),
            Some(JsValue::Undefined),
        ),
    ] {
        for tail in [false, true] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let (mut execution, id) = native_execution(
                &runtime,
                &mut context,
                name,
                input.try_clone().unwrap(),
                tail,
            );
            let owners = std::rc::Rc::strong_count(&runtime.0);
            #[cfg(feature = "profiling")]
            let profile = crate::engine::api::profiling::CostProfile::start();
            {
                let mut state = runtime.0.state.borrow_mut();
                assert!(
                    matches!(
                        super::execute_frame_in_state(&runtime, &mut state, &mut execution, id)
                            .unwrap(),
                        super::VmAction::Complete
                    ),
                    "{name} tail={tail}"
                );
                match &expected {
                    Some(expected) => {
                        assert_eq!(execution.pending.as_ref(), Some(expected), "{name}")
                    }
                    None => assert!(
                        matches!(execution.pending, Some(JsValue::Float(n)) if (0.0..1.0).contains(&n))
                    ),
                }
                assert_eq!(state.active_frames.len(), 1);
                assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
                assert!(!runtime.0.deferred_references.has_pending());
            }
            #[cfg(feature = "profiling")]
            {
                let events = profile.snapshot().owned_execution_events;
                assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
                assert_eq!(events.get("core.internal_native_body"), Some(&1));
                assert_eq!(events.get("native_state_body"), Some(&1));
                assert_eq!(events.get("runtime.clone"), None);
                assert_eq!(events.get("runtime.deferred.release"), None);
            }
            drop(execution);
            assert!(runtime.0.state.borrow().active_frames.is_empty());
            assert!(!runtime.is_poisoned());
        }
    }
}

#[test]
fn resident_predicate_releases_transient_aliases_before_collecting_published_frame_storage() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(marker) = context
        .eval("(()=>{let o={};o.self=o;return o})()")
        .unwrap()
    else {
        panic!("cycle");
    };
    let marker_id = marker.object_id();
    let Value::Object(garbage) = context
        .eval("(()=>{let o={};o.self=o;return o})()")
        .unwrap()
    else {
        panic!("cycle");
    };
    let garbage = garbage.into_handle();
    runtime.release_jsvalue(JsValue::Object(garbage)).unwrap();
    let (mut execution, id) = native_execution(
        &runtime,
        &mut context,
        "Number.isFinite",
        Value::Object(marker),
        false,
    );
    runtime.0.gc_pressure.remaining.set(0);
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        assert_eq!(execution.pending, Some(JsValue::Bool(false)));
        assert!(state.heap.object(marker_id).is_ok());
        assert!(state.heap.object(garbage).is_err());
        assert!(!runtime.0.gc_pressure.requested());
    }
    drop(execution);
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().heap.object(marker_id).is_err());
    assert_eq!(context.eval("1+1").unwrap(), Value::Int(2));
}

#[test]
fn resident_whole_predicate_family_ignores_coercion_and_handles_method_and_bound_calls() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(()=>{
        let calls=0, bad={valueOf(){calls++;throw 99;}};
        let holder={p:Number.isFinite};
        function tail(x){return holder.p(x);}
        let bound=Number.isNaN.bind(bad);
        let ok=!holder.p(bad)&&!tail(bad)&&bound(NaN)
            &&!Number.isInteger(1n)&&!Number.isSafeInteger(Symbol())
            &&!Number.isFinite('3')&&Function.prototype.call(bad, bad)===undefined;
        return ok&&calls===0;
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}
