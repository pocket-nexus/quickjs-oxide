//! Actual computed instructions share the read, conversion and callback kernels.
use super::read_completion_tests::{read_fixture, read_fixture_with_limits};
use crate::engine::{
    api::{Runtime, Value},
    atom::AtomIdx,
    code::exec_opcode::Opcode,
    heap::{GcPolicy, RawId},
    value::{
        JsValue,
        conversion::{
            NativeConversion,
            number::NumberStep,
            primitive::{PrimitiveResume, PrimitiveStep},
        },
    },
    vm::{
        Completion, ToPrimitiveHint,
        execute::{FallthroughPc, VmAction, execute_frame_in_state},
        execution::{ExecutionLimits, RunningExecution},
        frame::FrameId,
        proxy_get_driver::{Progress, StateNativeProgress},
        stack::FrameExecution,
    },
};

fn push(execution: &mut RunningExecution, id: FrameId, value: JsValue) {
    let frame = execution.frames.current_mut(id).unwrap();
    execution.slots.push(&mut frame.window, value).unwrap();
}
fn unreachable_cycle(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
) -> crate::engine::heap::ObjectId {
    let Value::Object(cycle) = context
        .eval("(()=>{let o={};o.self=o;return o})()")
        .unwrap()
    else {
        panic!("cycle")
    };
    let id = cycle.into_handle();
    runtime.release_jsvalue(JsValue::Object(id)).unwrap();
    id
}
fn spelling(state: &crate::engine::heap::runtime::RuntimeState, value: &JsValue) -> String {
    match value {
        JsValue::Undefined => "undefined".into(),
        JsValue::Int(value) => value.to_string(),
        JsValue::String(id) => state.heap.string(*id).unwrap().to_utf8_lossy(),
        JsValue::BigInt(id) => state.heap.bigint(*id).unwrap().to_string(),
        JsValue::ShortBigInt(value) => value.to_string(),
        _ => panic!("unexpected output {value:?}"),
    }
}

#[test]
fn computed_complete_domain_leaves_pressure_latched_without_public_roots() {
    for (source, key, expected) in [
        ("({x:7})", "'x'", "7"),
        ("({})", "'x'", "undefined"),
        (
            "Object.defineProperty({},'x',{get:undefined})",
            "'x'",
            "undefined",
        ),
        ("[7]", "0", "7"),
        ("[7]", "'length'", "1"),
        ("'abcd'", "0", "a"),
        ("'abcd'", "'length'", "4"),
        ("new Uint32Array([7])", "0", "7"),
        (
            "new BigUint64Array([18446744073709551615n])",
            "0",
            "18446744073709551615",
        ),
        ("new Uint32Array([7])", "'-0'", "undefined"),
        ("new Uint32Array([7])", "9", "undefined"),
        ("23", "'absent'", "undefined"),
        ("true", "'absent'", "undefined"),
        ("Symbol('leaf')", "'absent'", "undefined"),
        ("10000000000000000000000000000n", "'absent'", "undefined"),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let cycle = unreachable_cycle(&runtime, &mut context);
        let base = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
        let key = runtime.into_jsvalue(context.eval(key).unwrap()).unwrap();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o,k){return o[k]})",
            Opcode::GetArrayElDense,
        );
        push(&mut execution, id, base);
        push(&mut execution, id, key);
        let owners = std::rc::Rc::strong_count(&runtime.0);
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            VmAction::Complete
        ));
        assert_eq!(
            spelling(&state, execution.pending.as_ref().unwrap()),
            expected,
            "{source}"
        );
        assert!(state.heap.object(cycle).is_ok());
        assert!(runtime.0.gc_pressure.requested());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(!runtime.0.deferred_references.has_pending());
    }
}

#[test]
fn computed_getter_and_conversion_replies_keep_nested_parent_queries_installed() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{
        let keys=0, getters=0;
        const inner={get y(){getters++;return 7}};
        const key={ [Symbol.toPrimitive](hint){keys++; if(hint!=='string') throw hint; return 'y'} };
        const outer={get x(){return inner[key]}};
        const first={ [Symbol.toPrimitive](hint){keys++;return 'x'} };
        function read(o,k){return 5+o[k]}
        return read(outer,first)*100+keys*10+getters;
    })()"#).unwrap(), Value::Int(1221));
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn computed_method_and_update_preserve_receiver_and_direct_or_converted_key() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(()=>{
        let trace=''; let value=7;
        const object={tag:42, get x(){trace+='g';return function(){return this.tag}},
            get '0'(){trace+='r';return value},set '0'(v){trace+='w';value=v}};
        const key={toString(){trace+='k';return 0}};
        const a=object['x'](); const b=object[0]++; const c=object[key]++;
        return a===42&&b===7&&c===8&&value===9&&trace==='grwkrw';
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn computed_nullish_does_not_run_key_conversion_and_preserves_catchable_fault() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(()=>{let n=0;
        const key={toString(){n++;throw 'key'}};
        let a=false,b=false;
        try{null[key]}catch(e){a=e instanceof TypeError}
        try{undefined[key]++}catch(e){b=e instanceof TypeError}
        return n===0&&a&&b;
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn computed_proxy_conversion_and_throw_callbacks_are_not_replayed() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{let keys=0,gets=0,trace='';
        const key={toString(){keys++;return 'x'}};
        const object=new Proxy({x:7},{get(t,k,r){gets++;if(k!=='x'||r!==object)throw 'roles';return t[k]}});
        const a=object[key];
        const throwing=new Proxy({}, {get(){trace+='p';throw 13}});
        let b=0;try{b=5+throwing[key]}catch(e){b=e;trace+='c'}
        return a===7&&b===13&&keys===2&&gets===1&&trace==='pc';
    })()"#).unwrap(), Value::Bool(true));
}

#[test]
fn selected_shared_computed_word_finishes_without_reselection_or_leaf_poll() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let cycle = unreachable_cycle(&runtime, &mut context);
    let base = runtime
        .into_jsvalue(
            context
                .eval("(()=>{let a=new Uint32Array(new SharedArrayBuffer(4));a[0]=7;return a})()")
                .unwrap(),
        )
        .unwrap();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k){return o[k]})",
        Opcode::GetArrayElDense,
    );
    push(&mut execution, id, base);
    push(&mut execution, id, JsValue::Int(0));
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            VmAction::NativeProgress
        ));
        assert!(state.heap.object(cycle).is_ok());
    }
    let packet = execution.selected_native_query.take().unwrap();
    assert!(matches!(
        super::super::proxy_get_driver::resume_resident_boundary(&runtime, &mut execution, packet)
            .unwrap(),
        Progress::Resident
    ));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Int(7)
    );
    assert!(runtime.0.state.borrow().heap.object(cycle).is_ok());
    assert!(runtime.0.gc_pressure.requested());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn standalone_key_value_issues_identity_once_and_preserves_owned_string_symbol() {
    for source in [
        "7",
        "-0",
        "true",
        "null",
        "undefined",
        "12n",
        "'same'",
        "Symbol('same')",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let cycle = unreachable_cycle(&runtime, &mut context);
        let input = runtime.into_jsvalue(context.eval(source).unwrap()).unwrap();
        let original = match &input {
            JsValue::String(id) => Some(JsValue::String(*id)),
            JsValue::Symbol(id) => Some(JsValue::Symbol(*id)),
            _ => None,
        };
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(k){return {[k]:7}})",
            Opcode::ToPropKey,
        );
        push(&mut execution, id, input);
        let frame = execution.frames.current_mut(id).unwrap();
        let next = FallthroughPc::from_decoded(
            frame
                .executable
                .exec
                .decode_published(frame.resume_pc as u32)
                .unwrap(),
        );
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        let mut counter = 41;
        let mut state = runtime.0.state.borrow_mut();
        let atom_count = state.atoms.len();
        {
            let mut segment = FrameExecution::admit(&mut execution, id).unwrap();
            assert!(matches!(
                super::super::execute::computed_read::property_key(
                    &runtime,
                    &mut state,
                    &mut segment,
                    &mut counter,
                    next
                )
                .unwrap(),
                StateNativeProgress::Published
            ));
        }
        let frame = execution.frames.current_mut(id).unwrap();
        let output = execution.slots.peek(&frame.window, 0).unwrap();
        if let Some(original) = original {
            assert_eq!(output, &original);
        } else {
            assert!(matches!(output, JsValue::String(_)));
        }
        assert_eq!(counter, 42);
        assert_eq!(state.atoms.len(), atom_count);
        assert!(state.heap.object(cycle).is_ok());
        assert!(runtime.0.gc_pressure.requested());
    }
}

#[test]
fn standalone_identity_exhaustion_leaves_original_input_and_pc_untouched() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let input = runtime
        .into_jsvalue(context.eval("'original'").unwrap())
        .unwrap();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(k){return {[k]:7}})",
        Opcode::ToPropKey,
    );
    push(&mut execution, id, input);
    let frame = execution.frames.current_mut(id).unwrap();
    let pc = frame.resume_pc;
    let next =
        FallthroughPc::from_decoded(frame.executable.exec.decode_published(pc as u32).unwrap());
    let mut state = runtime.0.state.borrow_mut();
    let mut counter = u64::MAX;
    {
        let mut segment = FrameExecution::admit(&mut execution, id).unwrap();
        assert!(
            super::super::execute::computed_read::property_key(
                &runtime,
                &mut state,
                &mut segment,
                &mut counter,
                next
            )
            .is_err()
        );
    }
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(frame.resume_pc, pc);
    assert_eq!(execution.slots.depth(&frame.window), 1);
    assert!(matches!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        JsValue::String(_)
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn computed_string_atom_retain_overflow_preserves_original_window() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let base = runtime
        .into_jsvalue(context.eval("({overflow_computed_key:7})").unwrap())
        .unwrap();
    let key_owner = runtime
        .intern_property_key("overflow_computed_key")
        .unwrap();
    let key = runtime
        .into_jsvalue(context.eval("'overflow_computed_key'").unwrap())
        .unwrap();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k){return o[k]})",
        Opcode::GetArrayElDense,
    );
    push(&mut execution, id, base);
    push(&mut execution, id, key);
    let pc = execution.frames.current_mut(id).unwrap().resume_pc;
    let mut state = runtime.0.state.borrow_mut();
    let index = AtomIdx::from_raw(key_owner.atom().raw());
    let count = state
        .atoms
        .resolve(key_owner.atom())
        .unwrap()
        .ref_count
        .expect("live property atom");
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    let result = execute_frame_in_state(&runtime, &mut state, &mut execution, id);
    state.atoms.set_ref_count_for_test(index, count);
    assert!(result.unwrap_err().message().contains("overflow"));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert_eq!(execution.slots.depth(&frame.window), 2);
    assert!(!runtime.is_poisoned());
}

#[test]
fn computed_getter_reservation_failure_keeps_parent_and_original_key_armed() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let base=runtime.into_jsvalue(context.eval("Object.defineProperty({},'true',{get:function(){let a=1,b=2,c=3,d=4;return a+b+c+d}})").unwrap()).unwrap();
    let object = match &base {
        JsValue::Object(id) => *id,
        _ => panic!("object"),
    };
    let (mut execution, id) = read_fixture_with_limits(
        &runtime,
        &mut context,
        "(function(o,k){return o[k]})",
        Opcode::GetArrayElDense,
        ExecutionLimits {
            slots: 4,
            ..ExecutionLimits::default()
        },
    );
    push(&mut execution, id, base);
    push(&mut execution, id, JsValue::Bool(true));
    let pc = execution.frames.current_mut(id).unwrap().resume_pc;
    let mut state = runtime.0.state.borrow_mut();
    assert!(execute_frame_in_state(&runtime, &mut state, &mut execution, id).is_err());
    assert_eq!(execution.frames.current_id(), Some(id));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert!(!frame.cold.has_pending_query());
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Bool(true)
    );
    assert_eq!(
        execution.slots.peek(&frame.window, 1).unwrap(),
        &JsValue::Object(object)
    );
    assert_eq!(state.heap.object_strong_count(object), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn computed_getter_fatal_retirement_stops_before_pending_or_child_publication() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let base = runtime
        .into_jsvalue(
            context
                .eval("Object.defineProperty({},'true',{get:function(){throw 'must not run'}})")
                .unwrap(),
        )
        .unwrap();
    let object = match &base {
        JsValue::Object(id) => *id,
        _ => panic!("object"),
    };
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k){return o[k]})",
        Opcode::GetArrayElDense,
    );
    push(&mut execution, id, base);
    push(&mut execution, id, JsValue::Bool(true));
    let pc = execution.frames.current_mut(id).unwrap().resume_pc;
    let invalid = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    for value in [invalid, later] {
        state
            .heap
            .queue_release_for_test(RawId::Object(value))
            .unwrap();
    }
    state
        .heap
        .set_strong_count_for_test(RawId::Object(invalid), 1);
    assert!(execute_frame_in_state(&runtime, &mut state, &mut execution, id).is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(execution.frames.current_id(), Some(id));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert!(!frame.cold.has_pending_query());
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Bool(true)
    );
    assert_eq!(
        execution.slots.peek(&frame.window, 1).unwrap(),
        &JsValue::Object(object)
    );
    assert_eq!(state.heap.object_strong_count(object), Ok(3));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
}

#[test]
fn failed_primitive_and_number_factory_facts_survive_guard_retirement_and_adaptation() {
    for number in [false, true] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let input = runtime
            .into_jsvalue(context.eval("({[Symbol.toPrimitive]:1})").unwrap())
            .unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let step = if number {
            let NumberStep::Read { mut resume } = NumberStep::start_jsvalue_in_state(
                &mut state,
                &runtime.0.poisoned,
                context.realm,
                input,
            )
            .unwrap() else {
                panic!("number read")
            };
            let (object, _) = resume.take_read_in_state();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
                .unwrap();
            let next = resume
                .resume_in_state(
                    &mut state,
                    &runtime.0.poisoned,
                    Completion::Return(JsValue::Int(1)),
                )
                .unwrap();
            assert!(matches!(
                &next,
                NumberStep::CyclePublished(NativeConversion::Throw(JsValue::Object(_)))
            ));
            crate::engine::vm::proxy_get_driver::Step::try_from(next).unwrap()
        } else {
            let PrimitiveStep::Get { mut resume } = PrimitiveResume::start_in_state(
                &mut state,
                &runtime.0.poisoned,
                context.realm,
                input,
                ToPrimitiveHint::String,
            )
            .unwrap() else {
                panic!("primitive read")
            };
            let (object, _) = resume.take_get_in_state();
            state
                .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
                .unwrap();
            let next = resume
                .resume_in_state(
                    &mut state,
                    &runtime.0.poisoned,
                    Completion::Return(JsValue::Int(1)),
                )
                .unwrap();
            assert!(matches!(
                &next,
                PrimitiveStep::CyclePublished(Completion::Throw(JsValue::Object(_)))
            ));
            crate::engine::vm::proxy_get_driver::Step::try_from(next).unwrap()
        };
        match step {
            crate::engine::vm::proxy_get_driver::Step::CyclePublishedComplete(Some(
                Completion::Throw(value),
            ))
            | crate::engine::vm::proxy_get_driver::Step::CyclePublishedNumber(Some(
                NativeConversion::Throw(value),
            )) => state
                .release_owned_jsvalue(&runtime.0.poisoned, value)
                .unwrap(),
            _ => panic!("publication fact was erased"),
        }
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn failed_object_key_publishes_throw_at_fault_before_actual_error_pressure_service() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let cycle = unreachable_cycle(&runtime, &mut context);
    let base = runtime
        .into_jsvalue(context.eval("({x:7})").unwrap())
        .unwrap();
    let key = runtime
        .into_jsvalue(context.eval("({[Symbol.toPrimitive]:1})").unwrap())
        .unwrap();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k){return o[k]})",
        Opcode::GetArrayElDense,
    );
    push(&mut execution, id, base);
    push(&mut execution, id, key);
    let pc = execution.frames.current_mut(id).unwrap().resume_pc;
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    let mut state = runtime.0.state.borrow_mut();
    assert!(matches!(
        execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
        VmAction::Throw
    ));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    let JsValue::Object(error) = execution.slots.peek(&frame.window, 0).unwrap() else {
        panic!("thrown Error owner")
    };
    assert!(state.heap.object(*error).is_ok());
    assert!(state.heap.object(cycle).is_err());
    assert!(!runtime.0.gc_pressure.requested());
    assert!(!runtime.is_poisoned());
}
