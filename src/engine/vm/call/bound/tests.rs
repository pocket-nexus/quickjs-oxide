use super::*;
use crate::engine::{
    api::{Runtime, Value},
    code::exec_opcode::Opcode,
    heap::{ObjectPayload, RawId},
    object::ObjectRef,
    vm::{
        Completion,
        execute::{VmAction, execute_frame_in_state},
        property_driver::read_completion_tests::read_fixture,
    },
};

fn object(context: &mut crate::engine::api::Context, source: &str) -> ObjectRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("object fixture")
    };
    object
}

fn bound_facts(state: &RuntimeState, bound: ObjectId) -> (ObjectId, JsValue, Vec<JsValue>) {
    let ObjectPayload::BoundFunction {
        target,
        this_value,
        arguments,
    } = &state.heap.object(bound).unwrap().payload
    else {
        panic!("Bound fixture")
    };
    (
        *target,
        JsValue::from_raw(this_value.clone()).unwrap(),
        arguments
            .iter()
            .map(|raw| JsValue::from_raw(raw.clone()).unwrap())
            .collect(),
    )
}

fn inputs(bound: &ObjectRef, receiver: JsValue, arguments: Vec<JsValue>) -> RawCallbackInputs {
    let function = bound.try_clone().unwrap().into_execution_handle();
    RawCallbackInputs::new(function, receiver, arguments)
}

#[test]
fn bound_call_all_four_opcodes_use_the_resident_owner_and_reply_path() {
    fn published_tail_caller(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
        method: bool,
    ) -> (
        crate::engine::vm::execution::RunningExecution,
        crate::engine::vm::frame::FrameId,
    ) {
        use crate::engine::{
            code::{
                bytecode::Instruction,
                function::{UnlinkedFunction, metadata::FunctionMetadata},
            },
            vm::{
                call::CallableExecution,
                execution::{ExecutionLimits, RunningExecution},
            },
        };
        // The source compiler keeps `call; return` as two instructions.
        // Publish the supported tail opcode through the real verifier/loader.
        let prefix = if method { 3 } else { 2 };
        let mut instructions = vec![Instruction::Undefined; prefix];
        instructions.push(if method {
            Instruction::TailCallMethod(1)
        } else {
            Instruction::TailCall(1)
        });
        let bytecode = runtime
            .publish_unlinked_function(
                context.realm,
                UnlinkedFunction::fixture(
                    instructions,
                    Vec::new(),
                    FunctionMetadata {
                        max_stack: prefix as u16,
                        strict: true,
                        ..Default::default()
                    },
                ),
            )
            .unwrap();
        let caller = runtime
            .new_bytecode_closure(context.realm, &bytecode)
            .unwrap();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = runtime.bytecode_for_callable(&caller).unwrap()
        else {
            unreachable!()
        };
        let entry = crate::engine::vm::root_call::prepare_call(
            runtime,
            context.realm,
            &caller,
            JsValue::Undefined,
            JsValue::Undefined,
            Vec::new(),
            bytecode,
            closure_slots,
        )
        .unwrap();
        let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
        let parent = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
        let frame = execution.frames.current_mut(parent).unwrap();
        frame.resume_pc = frame.executable.exec.exec_pc(prefix as u32).unwrap() as usize;
        let actual = frame
            .executable
            .exec
            .decode_published(frame.resume_pc as u32)
            .unwrap()
            .opcode;
        assert_eq!(
            actual,
            if method {
                Opcode::TailCallMethod
            } else {
                Opcode::TailCall
            }
        );
        (execution, parent)
    }
    for (source, opcode, method) in [
        ("(function(f){let n=f(3);return n})", Opcode::Call, false),
        (
            "(function(o){let n=o.f(3);return n})",
            Opcode::CallMethod,
            true,
        ),
        (
            "(function(f){'use strict';return f(3)})",
            Opcode::TailCall,
            false,
        ),
        (
            "(function(o){'use strict';return o.f(3)})",
            Opcode::TailCallMethod,
            true,
        ),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let bound = object(
            &mut context,
            "(function(a,b,c){'use strict';return this+a*100+b*10+c}).bind(1000,1).bind(9000,2)",
        );
        let (mut execution, parent) = if matches!(opcode, Opcode::TailCall | Opcode::TailCallMethod)
        {
            published_tail_caller(&runtime, &mut context, method)
        } else {
            read_fixture(&runtime, &mut context, source, opcode)
        };
        let function = bound.try_clone().unwrap().into_execution_handle();
        let receiver = if method {
            Some(runtime.new_object(None).unwrap().into_execution_handle())
        } else {
            None
        };
        let frame = execution.frames.current_mut(parent).unwrap();
        if let Some(receiver) = receiver {
            execution
                .slots
                .push(&mut frame.window, JsValue::Object(receiver))
                .unwrap();
        }
        execution
            .slots
            .push(&mut frame.window, JsValue::Object(function))
            .unwrap();
        execution
            .slots
            .push(&mut frame.window, JsValue::Int(3))
            .unwrap();
        let runtime_owners = Rc::strong_count(&runtime.0);
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                execute_frame_in_state(&runtime, &mut state, &mut execution, parent).unwrap(),
                VmAction::Complete
            ));
            assert_eq!(execution.pending, Some(JsValue::Int(1123)), "{opcode:?}");
            assert_eq!(
                Rc::strong_count(&runtime.0),
                runtime_owners,
                "no public roots during Bound entry/reply"
            );
            if let Some(receiver) = receiver {
                assert!(
                    state.heap.object(receiver).is_err(),
                    "superseded method receiver retires"
                );
            }
            assert!(!runtime.0.deferred_references.has_pending());
        }
    }
}

#[test]
fn bound_call_all_receiver_and_argument_representations_preserve_identity_and_order() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{let o={},s=Symbol('s'),r='x'.repeat(4096)+'y'.repeat(4096),b=10000000000000000000000000000n;let values=[undefined,null,false,true,17,-0,NaN,r,s,3n,b,o];for(let receiver of values){let f=(function(){'use strict';return Object.is(this,receiver)&&arguments.length===values.length&&values.every((v,i)=>Object.is(arguments[i],v))}).bind(receiver,...values.slice(0,6)).bind(o,...values.slice(6,10));if(!f(...values.slice(10)))return false}return true})()").unwrap(), Value::Bool(true));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn bound_call_terminal_native_special_bytecode_and_proxy_effects_are_consumed_once() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    for (source, expected) in [
        ("Math.imul.bind(null,6).bind(null,7)()", Value::Int(42)),
        (
            "String.fromCharCode.bind(null,65).bind(null,66)(67)",
            Value::String(crate::engine::value::JsString::from_static("ABC")),
        ),
        (
            "Array.prototype.join.bind([1,2,3],'-')()",
            Value::String(crate::engine::value::JsString::from_static("1-2-3")),
        ),
        (
            "(function* named(a,b){yield a*10+b}).bind(null,4).bind(null,2)().next().value",
            Value::Int(42),
        ),
        (
            "(()=>{let n=0;let p=new Proxy(function(a,b){return this.v+a*10+b},{apply(t,r,a){n++;return Reflect.apply(t,r,a)}});let f=p.bind({v:100},4).bind({v:999},2);return f()+n})()",
            Value::Int(143),
        ),
        (
            "(()=>{let p=Proxy.revocable(function(){},{}),f=p.proxy.bind(null);p.revoke();try{f()}catch(e){return e instanceof TypeError}})()",
            Value::Bool(true),
        ),
    ] {
        assert_eq!(context.eval(source).unwrap(), expected, "{source}");
    }
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn bound_getters_and_toprimitive_callbacks_preserve_the_actual_receiver_hint_and_effects() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    for source in [
        "(()=>{let n=0,r={v:40};let g=(function(a){n++;return this.v+a}).bind(r,2).bind({v:100});let o=Object.defineProperty({},'x',{get:g});const {x:v}=o;return v===42&&n===1})()",
        "(()=>{let n=0,r={v:42};let m=(function(h){n++;return h==='number'?this.v:0}).bind(r);let o={[Symbol.toPrimitive]:m};return Math.abs(o)===42&&n===1})()",
        "(()=>{let log='';let o={[Symbol.toPrimitive]:(function(h){log+=h;return this.text}).bind({text:' 42 '})};let v=String.prototype.trim.call(o);return v==='42'&&log==='string'})()",
    ] {
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
    }
}

#[test]
fn bound_call_observes_evaluation_and_target_backtrace_without_a_bound_activation() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{let log='';let f=(function actualTarget(a,b){log+='T';throw new Error(log)}).bind(null,1).bind(null);let o={get f(){log+='G';return f}};try{o.f((log+='A',2))}catch(e){return e.message==='GAT'&&e.stack.indexOf('actualTarget')>=0&&e.stack.indexOf('at bound ')<0}})()").unwrap(), Value::Bool(true));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn bound_snapshot_failed_argument_promotion_rolls_back_receiver_arguments_and_target() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let bound = object(
        &mut context,
        "(()=>{let r={},a={},b={};return (function(){}).bind(r,a,b)})()",
    );
    let original = inputs(&bound, JsValue::Null, Vec::new());
    let mut state = runtime.0.state.borrow_mut();
    let (target, JsValue::Object(receiver), arguments) = bound_facts(&state, bound.object_id())
    else {
        panic!("object receiver")
    };
    let [JsValue::Object(first), JsValue::Object(blocked)] = arguments.as_slice() else {
        panic!("object arguments")
    };
    let counts =
        [target, receiver, *first, *blocked].map(|id| state.heap.object_strong_count(id).unwrap());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(*blocked), u32::MAX);
    let mut owner = RawCallbackGuard::new(&mut state, &runtime.0.poisoned, original);
    let (state, actual) = owner.parts();
    assert!(
        actual
            .select_callback_in_state(&runtime, state, context.realm)
            .is_err()
    );
    assert_eq!(actual.selected_callee, Some(bound.object_id()));
    assert_eq!(actual.receiver, Some(JsValue::Null));
    assert!(actual.arguments.is_empty());
    for (id, count) in [target, receiver, *first].into_iter().zip(counts) {
        assert_eq!(state.heap.object_strong_count(id), Ok(count));
    }
    assert!(!runtime.is_poisoned());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(*blocked), counts[3]);
    owner.retire().unwrap();
}

#[test]
fn bound_final_ordinary_callee_promotion_is_a_genuine_recoverable_owner_boundary() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let bound = object(&mut context, "(function(x){'use strict';return x}).bind(7)");
    let argument = runtime.new_object(None).unwrap().into_execution_handle();
    let original = inputs(&bound, JsValue::Null, vec![JsValue::Object(argument)]);
    let mut state = runtime.0.state.borrow_mut();
    let (target, _, _) = bound_facts(&state, bound.object_id());
    let before = state.heap.object_strong_count(target).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(target), u32::MAX - 1);
    let mut owner = RawCallbackGuard::new(&mut state, &runtime.0.poisoned, original);
    {
        let (state, actual) = owner.parts();
        assert!(
            actual
                .select_callback_in_state(&runtime, state, context.realm)
                .is_err()
        );
        assert_eq!(
            actual.selected_callee,
            Some(target),
            "target promotion completed before final callback retain"
        );
        assert_eq!(actual.receiver, Some(JsValue::Int(7)));
        assert!(actual.callback_callee.is_none());
        assert_eq!(state.heap.object_strong_count(target), Ok(u32::MAX));
        assert_eq!(state.heap.object_strong_count(argument), Ok(1));
        assert!(!runtime.is_poisoned());
        state
            .heap
            .set_strong_count_for_test(RawId::Object(target), before + 1);
    }
    owner.retire().unwrap();
    drop(owner);
    assert!(state.heap.object(argument).is_err());
    assert_eq!(state.heap.object_strong_count(target), Ok(before));
}

#[test]
fn bound_resident_admission_uses_live_payload_edges_without_obsolete_bytecode_or_global_roots() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let bound = object(
        &mut context,
        "(function target(){'use strict';return 42}).bind(null)",
    );
    let (mut execution, parent) = read_fixture(
        &runtime,
        &mut context,
        "(function(f){let n=f();return n})",
        Opcode::Call,
    );
    let owned = bound.try_clone().unwrap().into_execution_handle();
    let frame = execution.frames.current_mut(parent).unwrap();
    execution
        .slots
        .push(&mut frame.window, JsValue::Object(owned))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let (target, _, _) = bound_facts(&state, bound.object_id());
    let ObjectPayload::BytecodeFunction { bytecode, .. } =
        state.heap.object(target).unwrap().payload
    else {
        panic!("target bytecode")
    };
    let global = state.heap.context(context.realm).unwrap().global_object;
    let bytecode_count = state
        .heap
        .strong_count(RawId::FunctionBytecode(bytecode))
        .unwrap();
    let global_count = state.heap.object_strong_count(global).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::FunctionBytecode(bytecode), u32::MAX);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(global), u32::MAX);
    assert!(matches!(
        execute_frame_in_state(&runtime, &mut state, &mut execution, parent).unwrap(),
        VmAction::Complete
    ));
    assert_eq!(execution.pending, Some(JsValue::Int(42)));
    assert_eq!(
        state.heap.strong_count(RawId::FunctionBytecode(bytecode)),
        Ok(u32::MAX)
    );
    assert_eq!(state.heap.object_strong_count(global), Ok(u32::MAX));
    state
        .heap
        .set_strong_count_for_test(RawId::FunctionBytecode(bytecode), bytecode_count);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(global), global_count);
    assert!(!runtime.is_poisoned());
}

#[test]
fn bound_snapshot_fatal_rollback_quarantines_the_promoted_suffix_before_outer_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let bound = object(
        &mut context,
        "(()=>{let r={},a={},b={};return (function(){}).bind(r,a,b)})()",
    );
    let original = inputs(&bound, JsValue::Null, Vec::new());
    let older = runtime.new_object(None).unwrap().into_execution_handle();
    let later = runtime.new_object(None).unwrap().into_execution_handle();
    let mut state = runtime.0.state.borrow_mut();
    let (target, JsValue::Object(receiver), arguments) = bound_facts(&state, bound.object_id())
    else {
        panic!("object receiver")
    };
    let [JsValue::Object(first), JsValue::Object(blocked)] = arguments.as_slice() else {
        panic!("object arguments")
    };
    let counts = [target, receiver, *first].map(|id| state.heap.object_strong_count(id).unwrap());
    state
        .heap
        .queue_release_for_test(RawId::Object(older))
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(later))
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(older), 1);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(*blocked), u32::MAX);
    let mut owner = RawCallbackGuard::new(&mut state, &runtime.0.poisoned, original);
    let (state, actual) = owner.parts();
    assert!(matches!(
        actual.select_callback_in_state(&runtime, state, context.realm),
        Err(RuntimeError::Poisoned)
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(receiver), Ok(counts[1]));
    assert_eq!(
        state.heap.object_strong_count(*first),
        Ok(counts[2] + 1),
        "argument guard did not cross poison"
    );
    assert_eq!(
        state.heap.object_strong_count(target),
        Ok(counts[0] + 1),
        "target guard did not cross poison"
    );
    assert_eq!(state.heap.strong_count(RawId::Object(later)), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
    assert_eq!(actual.selected_callee, Some(bound.object_id()));
}

#[test]
fn bound_argument_overflow_uses_caller_realm_and_retires_both_buffers_before_error() {
    let runtime = Runtime::new();
    let caller = runtime.new_context().unwrap();
    let other = runtime.new_context().unwrap();
    let prefix = runtime.new_object(None).unwrap().into_execution_handle();
    let suffix = runtime.new_object(None).unwrap().into_execution_handle();
    let mut bound = (0..65_534).map(|_| JsValue::Undefined).collect::<Vec<_>>();
    bound[0] = JsValue::Object(prefix);
    let mut state = runtime.0.state.borrow_mut();
    let NativeConversion::Throw(JsValue::Object(error)) = state
        .concatenate_bound_arguments_jsvalue(
            &runtime.0.poisoned,
            caller.realm,
            bound,
            vec![JsValue::Object(suffix)],
        )
        .unwrap()
    else {
        panic!("argument overflow")
    };
    assert!(state.heap.object(prefix).is_err());
    assert!(state.heap.object(suffix).is_err());
    let error_data = state.heap.object(error).unwrap();
    let prototype = state.heap.shape(error_data.shape).unwrap().prototype();
    assert_eq!(
        prototype,
        state
            .heap
            .context(caller.realm)
            .unwrap()
            .native_error_prototypes[NativeErrorKind::Internal.index()]
    );
    assert_ne!(
        prototype,
        state
            .heap
            .context(other.realm)
            .unwrap()
            .native_error_prototypes[NativeErrorKind::Internal.index()]
    );
    // The actual producer Error stays owned by the canonical Query while its
    // publication fact crosses the immediate Identity parent exactly once.
    use crate::engine::vm::{
        frame::{ReturnOwner, ReturnTarget, ReturnValue},
        proxy_get_driver::{QueryStorage, Resume, StateEffect, Step, resident_query},
    };
    let mut storage = QueryStorage::default();
    let mut query = resident_query(
        &mut storage,
        caller.realm,
        ReturnTarget {
            owner: ReturnOwner::Root,
            value_use: ReturnValue::Push,
            tail: false,
            operation: None,
        },
        None,
    );
    let mut step = Step::CyclePublishedPrimitiveReply {
        value: Some(Completion::Throw(JsValue::Object(error))),
        resume: Some(Resume::Identity),
    };
    let first = query
        .advance_raw_in_state(&runtime, &mut state, &mut step)
        .unwrap();
    assert!(first.cycle_published);
    assert!(matches!(first.effect, StateEffect::Complete));
    assert_eq!(state.heap.object_strong_count(error), Ok(1));
    let second = query
        .advance_raw_in_state(&runtime, &mut state, &mut step)
        .unwrap();
    assert!(!second.cycle_published);
    step.retire_raw_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    query
        .recycle_in_state(&runtime, &mut state, &mut storage)
        .unwrap();
    assert!(state.heap.object(error).is_err());
}

#[test]
fn bound_argument_overflow_fatal_cleanup_wins_before_diagnostic_and_caller_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let older = runtime.new_object(None).unwrap().into_execution_handle();
    let first = runtime.new_object(None).unwrap().into_execution_handle();
    let suffix = runtime.new_object(None).unwrap().into_execution_handle();
    let mut bound = (0..65_534).map(|_| JsValue::Undefined).collect::<Vec<_>>();
    bound[0] = JsValue::Object(first);
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .queue_release_for_test(RawId::Object(older))
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(older), 1);
    let objects = state.heap.counts().object_nodes;
    assert!(
        state
            .concatenate_bound_arguments_jsvalue(
                &runtime.0.poisoned,
                context.realm,
                bound,
                vec![JsValue::Object(suffix)]
            )
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.strong_count(RawId::Object(first)), Ok(0));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(
        state.heap.counts().object_nodes,
        objects,
        "no diagnostic allocation after fatal prefix"
    );
}

#[test]
fn synchronous_function_call_bridge_normalizes_each_forwarded_bound_domain_once() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let bound = object(
        &mut context,
        "(()=>{let f=(function(a,b){'use strict';return this.v+a*10+b}).bind({v:100},4).bind({v:999},2);return Function.prototype.call.bind(f,{v:0})})()",
    );
    let callable = runtime.as_callable(&bound).unwrap().unwrap();
    let before = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(bound.object_id())
        .unwrap();
    assert!(matches!(
        runtime
            .call_internal_jsvalue(context.realm, &callable, JsValue::Null, Vec::new())
            .unwrap(),
        Completion::Return(JsValue::Int(142))
    ));
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(bound.object_id()),
        Ok(before)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}
