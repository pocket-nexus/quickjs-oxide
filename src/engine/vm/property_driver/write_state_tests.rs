//! Actual PutField/PutArrayEl use the one raw Set/Query consumer.
use super::read_completion_tests::{read_fixture, read_fixture_with_limits};
use crate::engine::{
    api::{Runtime, Value},
    code::exec_opcode::Opcode,
    heap::GcPolicy,
    value::JsValue,
    vm::{
        execute::{VmAction, execute_frame_in_state, execute_frame_in_state_with_identity},
        execution::RunningExecution,
        frame::FrameId,
        stack::FrameExecution,
    },
};

fn push(execution: &mut RunningExecution, id: FrameId, value: JsValue) {
    let frame = execution.frames.current_mut(id).unwrap();
    execution.slots.push(&mut frame.window, value).unwrap();
}

#[test]
fn strict_write_diagnostics_consume_real_suffix_at_full_verified_capacity() {
    for (source, opcode, base) in [
        (
            "(function(o,v){'use strict';return o.x=v})",
            Opcode::PutField,
            "null",
        ),
        (
            "(function(o,v){'use strict';return o.x=v})",
            Opcode::PutField,
            "Object.freeze({x:7})",
        ),
        (
            "(function(o,k,v){'use strict';return o[k]=v})",
            Opcode::PutArrayEl,
            "undefined",
        ),
        (
            "(function(o,k,v){'use strict';return o[k]=v})",
            Opcode::PutArrayEl,
            "Object.freeze({x:7})",
        ),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let base = runtime.into_jsvalue(context.eval(base).unwrap()).unwrap();
        let key = runtime.into_jsvalue(context.eval("'x'").unwrap()).unwrap();
        let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
        // Insert2/Insert3 preserved the real assignment-result role below the
        // original write suffix. This is the actual compiler-verified peak.
        push(&mut execution, id, JsValue::Int(99));
        push(&mut execution, id, base);
        if opcode == Opcode::PutArrayEl {
            push(&mut execution, id, key);
        } else {
            runtime.release_jsvalue(key).unwrap();
        }
        push(&mut execution, id, JsValue::Int(99));
        let pc = execution.frames.current_mut(id).unwrap().resume_pc;
        {
            let mut segment = FrameExecution::admit(&mut execution, id).unwrap();
            assert!(
                !segment.frame().transaction.slots().has_operand_capacity(1),
                "fixture must fill real verified capacity"
            );
        }
        assert!(
            !crate::engine::vm::proxy_get_driver::RawNativeQuery::has_cached_query_for_test(
                &execution.query_storage
            )
        );
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            VmAction::Throw
        ));
        assert!(
            crate::engine::vm::proxy_get_driver::RawNativeQuery::has_cached_query_for_test(
                &execution.query_storage
            ),
            "actual diagnostic must enter the common Query consumer"
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            execution.slots.peek(&frame.window, 1).unwrap(),
            &JsValue::Int(99)
        );
        let JsValue::Object(error) = execution.slots.peek(&frame.window, 0).unwrap() else {
            panic!("actual Error owner")
        };
        assert!(state.heap.object(*error).is_ok());
        assert!(execution.pending.is_none());
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn computed_write_key_conversion_precedes_nullish_failure_and_keeps_thrown_identity() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(()=>{
        let trace=''; const marker={};
        const key={ [Symbol.toPrimitive](hint){trace+=hint;return 'x'} };
        let a=false,b=false;
        try{null[key]=7}catch(e){a=e instanceof TypeError;trace+='a'}
        try{undefined[{toString(){trace+='k';throw marker}}]=8}catch(e){b=e===marker;trace+='b'}
        return a&&b&&trace==='stringakb';
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn static_and_computed_writes_preserve_all_owned_value_representations() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{
        const values=[undefined,null,true,7,1.5,'arena string',1n,123456789012345678901234567890n,Symbol('s'),{}];
        const direct={},computed={},dense=[]; let conversions=0;
        const key={toString(){conversions++;return 'x'}};
        for(let i=0;i<values.length;i++){
            const value=values[i];
            if((direct.x=value)!==value || (computed[key]=value)!==value || (dense[i]=value)!==value) return false;
            if(direct.x!==value || computed.x!==value || dense[i]!==value) return false;
        }
        return conversions===values.length;
    })()"#).unwrap(), Value::Bool(true));
    runtime.run_gc().unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn setter_callbacks_keep_nested_native_bound_proxy_and_throw_continuations() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{
        let trace='',sum=0,inner={};const marker={};
        const object={set x(v){trace+='o';inner.y=v;sum+=inner.y}};
        const bound=(function(v){trace+='b';sum+=v}).bind(null);
        Object.defineProperty(object,'b',{set:bound});
        Object.defineProperty(object,'n',{set:Number.isFinite});
        Object.defineProperty(object,'p',{set:new Proxy(function(v){trace+='p';sum+=v},{apply(f,t,a){trace+='a';return Reflect.apply(f,t,a)}})});
        Object.defineProperty(object,'t',{set(v){trace+='t';throw marker}});
        function write(o,k,v){return 10+(o[k]=v)}
        const a=write(object,'x',3),b=write(object,'b',4),c=write(object,'n',5),d=write(object,'p',6);
        let e=0;try{e=write(object,'t',7)}catch(error){if(error!==marker)return false;e=7;trace+='c'}
        return a===13&&b===14&&c===15&&d===16&&e===7&&sum===13&&trace==='obaptc';
    })()"#).unwrap(), Value::Bool(true));
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.is_poisoned());
}

#[test]
fn different_receiver_uses_own_rules_without_receiver_prototype_setter() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(()=>{
        let calls=0; const target={x:1};
        const proto={set x(v){calls++;throw 'wrong prototype walk'}};
        const receiver=Object.create(proto);
        const accepted=Reflect.set(target,'x',7,receiver);
        const accessor=Object.defineProperty({},'x',{set(v){calls++;},configurable:true});
        const no=Reflect.set(target,'x',8,accessor);
        const frozen=Object.preventExtensions({});
        const no2=Reflect.set(target,'x',9,frozen);
        return accepted&&receiver.x===7&&target.x===1&&!no&&!no2&&calls===0;
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn array_length_and_typed_conversion_keep_selected_effect_order_and_authoritative_storage() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{
        let trace='';const a=[1,2,3];
        const length={valueOf(){trace+='l';return 2}};a.length=length;
        const typed=new Uint8Array(1),value={valueOf(){trace+='v';return 257}};
        typed[0]=value;typed[-0]=3;
        const invalid={valueOf(){trace+='i';return 4}};typed['-0']=invalid;
        const receiver={};const different=Reflect.set(typed,'0',7,receiver);
        return a.length===2&&a[1]===2&&a[2]===undefined&&typed[0]===3&&different&&receiver[0]===7&&trace==='llvi';
    })()"#).unwrap(), Value::Bool(true));
}

#[test]
fn mapped_arguments_and_primitive_virtual_writes_keep_storage_semantics() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(()=>{
        function mapped(a){arguments[0]=7;return a===7&&arguments[0]===7}
        function unmapped(a){'use strict';arguments[0]=7;return a===1&&arguments[0]===7}
        let stringError=false,primitiveError=false;
        try{(function(){'use strict';'abc'[0]='z'})()}catch(e){stringError=e instanceof TypeError}
        try{(function(){'use strict';(7).x=8})()}catch(e){primitiveError=e instanceof TypeError}
        return mapped(1)&&unmapped(1)&&stringError&&primitiveError;
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn local_nonallocating_write_keeps_unrelated_gc_pressure_latched_inside_state() {
    for (source, opcode) in [
        ("(function(o,v){o.x=v})", Opcode::PutField),
        ("(function(o,k,v){o[k]=v})", Opcode::PutArrayEl),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let Value::Object(cycle) = context
            .eval("(()=>{let o={};o.self=o;return o})()")
            .unwrap()
        else {
            panic!("cycle")
        };
        let cycle = cycle.into_handle();
        runtime.release_jsvalue(JsValue::Object(cycle)).unwrap();
        let input = runtime
            .into_jsvalue(context.eval("({x:7})").unwrap())
            .unwrap();
        // Prepare the public key before registering the actual execution.
        let key = if opcode == Opcode::PutArrayEl {
            Some(runtime.into_jsvalue(context.eval("'x'").unwrap()).unwrap())
        } else {
            None
        };
        let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
        push(&mut execution, id, JsValue::Int(9)); // compiler's preserved assignment role
        push(&mut execution, id, input);
        if let Some(key) = key {
            push(&mut execution, id, key);
        }
        push(&mut execution, id, JsValue::Int(9));
        let owners = std::rc::Rc::strong_count(&runtime.0);
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            VmAction::Complete
        ));
        assert!(state.heap.object(cycle).is_ok());
        assert!(runtime.0.gc_pressure.requested());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(!runtime.0.deferred_references.has_pending());
    }
}

#[test]
fn object_write_key_identity_exhaustion_keeps_original_verified_window_owners() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let base = runtime
        .into_jsvalue(context.eval("({x:7})").unwrap())
        .unwrap();
    let key = runtime
        .into_jsvalue(
            context
                .eval("({toString(){throw 'must not run'}})")
                .unwrap(),
        )
        .unwrap();
    let base_id = match &base {
        JsValue::Object(id) => *id,
        _ => unreachable!(),
    };
    let key_id = match &key {
        JsValue::Object(id) => *id,
        _ => unreachable!(),
    };
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k,v){return o[k]=v})",
        Opcode::PutArrayEl,
    );
    push(&mut execution, id, JsValue::Int(9));
    push(&mut execution, id, base);
    push(&mut execution, id, key);
    push(&mut execution, id, JsValue::Int(9));
    let pc = execution.frames.current_mut(id).unwrap().resume_pc;
    let mut identity = u64::MAX;
    let mut state = runtime.0.state.borrow_mut();
    let error = execute_frame_in_state_with_identity(
        &runtime,
        &mut state,
        &mut execution,
        id,
        &mut identity,
    )
    .unwrap_err();
    assert!(error.message().contains("conversion identity exhausted"));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert_eq!(execution.slots.depth(&frame.window), 4);
    assert_eq!(
        execution.slots.peek(&frame.window, 2).unwrap(),
        &JsValue::Object(base_id)
    );
    assert_eq!(
        execution.slots.peek(&frame.window, 1).unwrap(),
        &JsValue::Object(key_id)
    );
    assert_eq!(state.heap.object_strong_count(base_id), Ok(1));
    assert_eq!(state.heap.object_strong_count(key_id), Ok(1));
    assert!(execution.selected_native_query.is_none());
    assert_eq!(identity, u64::MAX);
    assert!(!runtime.is_poisoned());
}

#[test]
fn resident_array_set_child_counts_outer_native_before_typed_valueof_maximum() {
    use crate::engine::{
        api::error::NativeErrorKind,
        atom::AtomIdx,
        heap::{PropertySlot, RawId, RawValue},
        vm::execution::ExecutionLimits,
    };

    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(receiver) = context
        .eval(
            "Object.defineProperty(new Uint8Array(1),'length',{value:0,writable:true,configurable:true})",
        )
        .unwrap()
    else {
        panic!("typed Array receiver with an own ordinary length")
    };
    let Value::Object(argument) = context
        .eval(
            "globalThis.setChildConversionCalls=0;({valueOf(){setChildConversionCalls++;return 257}})",
        )
        .unwrap()
    else {
        panic!("actual object conversion argument")
    };
    let Value::Object(native) = context.eval("Array.prototype.push").unwrap() else {
        panic!("actual Array push native")
    };
    let method_key = runtime.intern_property_key("valueOf").unwrap();
    let (mut execution, id) = read_fixture_with_limits(
        &runtime,
        &mut context,
        "(function(o,v){return 99+o.push(v)})",
        Opcode::CallMethod,
        ExecutionLimits {
            frames: 2,
            ..ExecutionLimits::default()
        },
    );
    let originals = {
        let state = runtime.0.state.borrow();
        [
            receiver.object_id(),
            native.object_id(),
            argument.object_id(),
        ]
        .map(|object| state.heap.object_strong_count(object).unwrap())
    };
    // This is the real published method-call suffix, not Function.call or a
    // manufactured Query. One bytecode frame admits the native at limit two.
    push(&mut execution, id, JsValue::Int(99));
    for object in [
        receiver.object_id(),
        native.object_id(),
        argument.object_id(),
    ] {
        push(
            &mut execution,
            id,
            runtime.dup_jsvalue(&JsValue::Object(object)).unwrap(),
        );
    }
    let fault = execution.frames.current_mut(id).unwrap().resume_pc;
    let error = {
        let mut state = runtime.0.state.borrow_mut();
        let data = state.heap.object(argument.object_id()).unwrap();
        let index = state
            .heap
            .shape(data.shape)
            .unwrap()
            .find(AtomIdx::from_raw(method_key.atom().raw()))
            .unwrap();
        let PropertySlot::Data(RawValue::Object(method)) = &data.slots[index as usize] else {
            panic!("real stored valueOf callee")
        };
        let method = *method;
        let original = state.heap.object_strong_count(method).unwrap();
        let expected = state
            .heap
            .context(context.realm)
            .unwrap()
            .native_error_prototypes[NativeErrorKind::Internal.index()]
        .unwrap();
        assert_eq!(execution.frames.depth(), 1);
        assert!(execution.frames.can_push_with_continuations(0));
        assert!(!execution.frames.can_push_with_continuations(1));
        state
            .heap
            .set_strong_count_for_test(RawId::Object(method), u32::MAX);
        let result = execute_frame_in_state(&runtime, &mut state, &mut execution, id);
        let maximum = state.heap.object_strong_count(method);
        // Restore before any assertion can unwind through the real property.
        state
            .heap
            .set_strong_count_for_test(RawId::Object(method), original);
        assert!(matches!(result.unwrap(), VmAction::Throw));
        assert_eq!(maximum, Ok(u32::MAX), "valueOf must remain unvisited");
        assert_eq!(execution.frames.depth(), 1, "no child frame was installed");
        assert!(execution.selected_native_query.is_none());
        assert!(execution.pending.is_none());
        let frame = execution.frames.current_mut(id).unwrap();
        assert!(!frame.cold.has_pending_query());
        assert_eq!((frame.fault_pc, frame.resume_pc), (fault, fault));
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            execution.slots.peek(&frame.window, 1).unwrap(),
            &JsValue::Int(99)
        );
        let JsValue::Object(error) = execution.slots.peek(&frame.window, 0).unwrap() else {
            panic!("actual stack-overflow Error owner")
        };
        let error = *error;
        let data = state.heap.object(error).unwrap();
        assert_eq!(
            state.heap.shape(data.shape).unwrap().prototype(),
            Some(expected)
        );
        assert_eq!(
            [
                receiver.object_id(),
                native.object_id(),
                argument.object_id()
            ]
            .map(|object| state.heap.object_strong_count(object).unwrap()),
            originals,
            "selected typed conversion and native inputs must retire to original owners"
        );
        assert_eq!(state.heap.object_strong_count(method), Ok(original));
        assert_eq!(state.active_frames.len(), 1);
        assert_eq!(runtime.0.active_frame_depth.get(), 1);
        assert!(!runtime.is_poisoned());
        error
    };
    let Value::Object(error) = runtime.root_value(&JsValue::Object(error)).unwrap() else {
        unreachable!()
    };
    drop(execution);
    let message_key = runtime.intern_property_key("message").unwrap();
    let Value::String(message) = context.get_property(&error, &message_key).unwrap() else {
        panic!("overflow message")
    };
    assert_eq!(message.to_string(), "stack overflow");
    assert_eq!(
        context.eval("setChildConversionCalls").unwrap(),
        Value::Int(0)
    );
    let index = runtime.property_key_for_index(0).unwrap();
    assert_eq!(
        context.get_property(&receiver, &index).unwrap(),
        Value::Int(0)
    );
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn borrowed_set_vm_keeps_selected_callbacks_and_strict_rejection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    assert_eq!(
        context
            .eval(
                r#"
            (() => {
                let trace = '';
                const target = Object.create({set x(v) { trace += 's' + v; }});
                target[{toString() { trace += 'k'; return 'x'; }}] = 7;
                const proxy = new Proxy({}, {
                    set(t, k, v, receiver) {
                        trace += 'p' + v;
                        return Reflect.set(t, k, v, receiver);
                    }
                });
                proxy.x = 9;
                const frozen = Object.freeze({x: 1});
                frozen.x = 2;
                try { (function() { 'use strict'; frozen.x = 3; })(); }
                catch (e) { trace += e instanceof TypeError ? 't' : '?'; }
                return trace === 'ks7p9t' && proxy.x === 9 && frozen.x === 1;
            })()
        "#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn borrowed_set_vm_preserves_typed_conversion_reentry_and_throw() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    assert_eq!(
        context
            .eval(
                r#"
            (() => {
                const target = new Uint8Array(1), marker = {};
                let calls = 0;
                target[0] = {valueOf() { calls++; target[0] = 8; return 257; }};
                try { target[0] = {valueOf() { calls++; throw marker; }}; }
                catch (e) { if (e !== marker) return false; }
                return calls === 2 && target[0] === 1;
            })()
        "#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn local_writes_complete_without_query_storage_for_every_value_owner_and_base_alias() {
    use crate::engine::vm::proxy_get_driver::RawNativeQuery;
    for (source, opcode) in [
        ("(function(o,v){o.x=v})", Opcode::PutField),
        ("(function(o,k,v){o[k]=v})", Opcode::PutArrayEl),
    ] {
        for expression in [
            "undefined",
            "null",
            "true",
            "7",
            "1.5",
            "'arena value'",
            "1n",
            "123456789012345678901234567890n",
            "Symbol('value')",
            "({})",
            "<base>",
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            let Value::Object(base) = context.eval("({x:7})").unwrap() else {
                unreachable!()
            };
            let value = if expression == "<base>" {
                runtime
                    .dup_jsvalue(&JsValue::Object(base.object_id()))
                    .unwrap()
            } else {
                runtime
                    .into_jsvalue(context.eval(expression).unwrap())
                    .unwrap()
            };
            let expected = runtime.root_value(&value).unwrap();
            let preserved = runtime.dup_jsvalue(&value).unwrap();
            let receiver = runtime
                .dup_jsvalue(&JsValue::Object(base.object_id()))
                .unwrap();
            let key = if opcode == Opcode::PutArrayEl {
                Some(runtime.into_jsvalue(context.eval("'x'").unwrap()).unwrap())
            } else {
                None
            };
            let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
            assert!(!RawNativeQuery::has_cached_query_for_test(
                &execution.query_storage
            ));
            push(&mut execution, id, preserved);
            push(&mut execution, id, receiver);
            if let Some(key) = key {
                push(&mut execution, id, key);
            }
            push(&mut execution, id, value);
            let owners = runtime.0.raw_execution_owners.get();
            {
                let mut state = runtime.0.state.borrow_mut();
                assert!(matches!(
                    execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
                    VmAction::Complete
                ));
                assert!(
                    !RawNativeQuery::has_cached_query_for_test(&execution.query_storage),
                    "{opcode:?} {expression} acquired a Query"
                );
                assert!(execution.selected_native_query.is_none());
                assert_eq!(runtime.0.raw_execution_owners.get(), owners);
                assert!(!runtime.0.deferred_references.has_pending());
            }
            drop(execution);
            let key = runtime.intern_property_key("x").unwrap();
            assert_eq!(context.get_property(&base, &key).unwrap(), expected);
            runtime.run_gc().unwrap();
            assert!(!runtime.is_poisoned());
        }
    }
}

#[test]
fn every_primitive_write_key_uses_local_state_without_query_or_extra_result_owner() {
    use crate::engine::vm::proxy_get_driver::RawNativeQuery;
    for expression in [
        "undefined",
        "null",
        "true",
        "0",
        "-0",
        "1.5",
        "'arena key'",
        "1n",
        "123456789012345678901234567890n",
        "Symbol('key')",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let receiver = runtime.into_jsvalue(context.eval("({})").unwrap()).unwrap();
        let key = runtime
            .into_jsvalue(context.eval(expression).unwrap())
            .unwrap();
        let Value::Object(value) = context.eval("({marker:7})").unwrap() else {
            unreachable!()
        };
        let value_id = value.object_id();
        let baseline = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(value_id)
            .unwrap();
        let preserved = runtime.dup_jsvalue(&JsValue::Object(value_id)).unwrap();
        let assigned = runtime.dup_jsvalue(&JsValue::Object(value_id)).unwrap();
        // An external receiver root lets the stored edge survive fixture cleanup.
        let receiver_root = runtime.root_value(&receiver).unwrap();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o,k,v){o[k]=v})",
            Opcode::PutArrayEl,
        );
        push(&mut execution, id, preserved);
        push(&mut execution, id, receiver);
        push(&mut execution, id, key);
        push(&mut execution, id, assigned);
        let mut identity = 41;
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                execute_frame_in_state_with_identity(
                    &runtime,
                    &mut state,
                    &mut execution,
                    id,
                    &mut identity
                )
                .unwrap(),
                VmAction::Complete
            ));
            assert_eq!(identity, 41, "primitive key created an operation identity");
            assert!(
                !RawNativeQuery::has_cached_query_for_test(&execution.query_storage),
                "{expression} acquired a Query"
            );
            assert_eq!(
                state.heap.object_strong_count(value_id),
                Ok(baseline + 1),
                "only the actual stored edge remains"
            );
            assert!(!runtime.is_poisoned());
        }
        drop(execution);
        drop(receiver_root);
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(value_id),
            Ok(baseline)
        );
    }
}

#[test]
fn real_write_effects_publish_the_existing_query_after_local_selection() {
    use crate::engine::vm::proxy_get_driver::RawNativeQuery;
    for (base, key, value) in [
        ("({set x(v){globalThis.prequerySetter=v}})", "'x'", "7"),
        ("new Proxy({},{set(){return true}})", "'x'", "7"),
        ("new Uint8Array(1)", "0", "({valueOf(){return 257}})"),
        ("[1,2,3]", "'length'", "({valueOf(){return 2}})"),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let base = runtime.into_jsvalue(context.eval(base).unwrap()).unwrap();
        let key = runtime.into_jsvalue(context.eval(key).unwrap()).unwrap();
        let value = runtime.into_jsvalue(context.eval(value).unwrap()).unwrap();
        let preserved = runtime.dup_jsvalue(&value).unwrap();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o,k,v){o[k]=v})",
            Opcode::PutArrayEl,
        );
        assert!(!RawNativeQuery::has_cached_query_for_test(
            &execution.query_storage
        ));
        push(&mut execution, id, preserved);
        push(&mut execution, id, base);
        push(&mut execution, id, key);
        push(&mut execution, id, value);
        {
            let mut state = runtime.0.state.borrow_mut();
            let _ = execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap();
            let published = execution
                .frames
                .current_mut(id)
                .unwrap()
                .cold
                .has_pending_query()
                || execution.selected_native_query.is_some()
                || RawNativeQuery::has_cached_query_for_test(&execution.query_storage);
            assert!(
                published,
                "actual setter/Proxy/typed/length wait did not use Query storage"
            );
            assert!(!runtime.is_poisoned());
        }
        drop(execution);
        assert_eq!(runtime.0.raw_execution_owners.get(), 0);
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn primitive_key_checked_duplicate_failure_preserves_window_identity_and_no_query() {
    use crate::engine::{heap::RawId, vm::proxy_get_driver::RawNativeQuery};
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let base = runtime
        .into_jsvalue(context.eval("({x:7})").unwrap())
        .unwrap();
    let key = runtime
        .into_jsvalue(context.eval("'checked_prequery_key'").unwrap())
        .unwrap();
    let JsValue::String(key_id) = &key else {
        unreachable!()
    };
    let key_id = *key_id;
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k,v){return o[k]=v})",
        Opcode::PutArrayEl,
    );
    push(&mut execution, id, JsValue::Int(9));
    push(&mut execution, id, base);
    push(&mut execution, id, key);
    push(&mut execution, id, JsValue::Int(9));
    let pc = execution.frames.current_mut(id).unwrap().resume_pc;
    let mut identity = 41;
    let mut state = runtime.0.state.borrow_mut();
    let raw = RawId::String(key_id);
    let original = state.heap.strong_count(raw).unwrap();
    state.heap.set_strong_count_for_test(raw, u32::MAX);
    let result = execute_frame_in_state_with_identity(
        &runtime,
        &mut state,
        &mut execution,
        id,
        &mut identity,
    );
    let maximum = state.heap.strong_count(raw);
    state.heap.set_strong_count_for_test(raw, original);
    assert!(result.is_err());
    assert_eq!(maximum, Ok(u32::MAX));
    assert_eq!(identity, 41);
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert_eq!(execution.slots.depth(&frame.window), 4);
    assert_eq!(
        execution.slots.peek(&frame.window, 1).unwrap(),
        &JsValue::String(key_id)
    );
    assert!(!RawNativeQuery::has_cached_query_for_test(
        &execution.query_storage
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn fatal_local_value_retirement_quarantines_atom_before_later_owner_cleanup() {
    use crate::engine::vm::proxy_get_driver::RawNativeQuery;
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let property = runtime
        .intern_property_key("quarantined_prequery_key")
        .unwrap();
    let key = runtime
        .into_jsvalue(context.eval("'quarantined_prequery_key'").unwrap())
        .unwrap();
    let invalid = runtime.new_object(None).unwrap().into_handle();
    runtime.release_jsvalue(JsValue::Object(invalid)).unwrap();
    let untouched = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k,v){return o[k]=v})",
        Opcode::PutArrayEl,
    );
    // Inject one stale destructive value while an unrelated lower operand stays
    // owned by the real verified window. No fabricated guard or Query is used.
    push(&mut execution, id, JsValue::Object(untouched));
    push(&mut execution, id, JsValue::Null);
    push(&mut execution, id, key);
    push(&mut execution, id, JsValue::Object(invalid));
    let mut state = runtime.0.state.borrow_mut();
    let baseline = state
        .atoms
        .resolve(property.atom())
        .unwrap()
        .ref_count
        .unwrap();
    let result = execute_frame_in_state(&runtime, &mut state, &mut execution, id);
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(
        state.atoms.resolve(property.atom()).unwrap().ref_count,
        Some(baseline + 1),
        "atom suffix must remain quarantined after fatal value retirement"
    );
    assert_eq!(state.heap.object_strong_count(untouched), Ok(1));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(execution.slots.depth(&frame.window), 1);
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Object(untouched)
    );
    assert!(!RawNativeQuery::has_cached_query_for_test(
        &execution.query_storage
    ));
}
