//! Named reads use the complete canonical selector and the existing executor.
use super::{SelectedNamedRead, read_completion_tests::read_fixture};
use crate::engine::{
    api::{Runtime, Value},
    atom::AtomIdx,
    code::exec_opcode::Opcode,
    heap::{AutoInitProperty, GcPolicy, PropertySlot, RawId},
    object::shape::{PropertyFlags, ShapeEntry},
    value::JsValue,
    vm::{
        execute::{VmAction, execute_frame, execute_frame_in_state},
        execution::RunningExecution,
        frame::FrameId,
    },
};

fn push_input(execution: &mut RunningExecution, id: FrameId, input: JsValue) {
    let frame = execution.frames.current_mut(id).unwrap();
    execution.slots.push(&mut frame.window, input).unwrap();
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

#[test]
fn whole_named_leaf_reads_leave_unrelated_pressure_latched() {
    for (input, source, opcode, expected) in [
        (
            "({x:7})",
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
            "7",
        ),
        (
            "({})",
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
            "undefined",
        ),
        (
            "Object.defineProperty({},'x',{get:undefined})",
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
            "undefined",
        ),
        (
            "Object.defineProperty({},'x',{get:function(){return 7}})",
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
            "7",
        ),
        (
            "[1,2,3]",
            "(function(o){return o.length})",
            Opcode::GetFieldCached,
            "3",
        ),
        (
            "[7]",
            "(function(o){const {'0':v}=o;return v})",
            Opcode::GetField2Cached,
            "7",
        ),
        (
            "'abcd'",
            "(function(o){const {'0':v}=o;return v})",
            Opcode::GetField2Cached,
            "a",
        ),
        (
            "'abcd'",
            "(function(o){return o.length})",
            Opcode::GetFieldCached,
            "4",
        ),
        (
            "new Uint32Array([7])",
            "(function(o){const {'0':v}=o;return v})",
            Opcode::GetField2Cached,
            "7",
        ),
        (
            "new BigUint64Array([18446744073709551615n])",
            "(function(o){const {'0':v}=o;return v})",
            Opcode::GetField2Cached,
            "18446744073709551615",
        ),
        (
            "23",
            "(function(o){return o.missing})",
            Opcode::GetFieldCached,
            "undefined",
        ),
        (
            "true",
            "(function(o){return o.missing})",
            Opcode::GetFieldCached,
            "undefined",
        ),
        (
            "Symbol('leaf')",
            "(function(o){return o.missing})",
            Opcode::GetFieldCached,
            "undefined",
        ),
        (
            "10000000000000000000000000000n",
            "(function(o){return o.missing})",
            Opcode::GetFieldCached,
            "undefined",
        ),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let cycle = unreachable_cycle(&runtime, &mut context);
        let input = context.eval(input).unwrap();
        let input = runtime.into_jsvalue(input).unwrap();
        let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
        push_input(&mut execution, id, input);
        let owners = std::rc::Rc::strong_count(&runtime.0);
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(
                matches!(
                    execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
                    VmAction::Complete
                ),
                "{source}"
            );
            assert!(
                state.heap.object(cycle).is_ok(),
                "{source} must not poll pressure"
            );
            assert!(runtime.0.gc_pressure.requested());
            let output = execution.pending.as_ref().unwrap();
            let actual = match output {
                JsValue::Undefined => "undefined".to_owned(),
                JsValue::Int(value) => value.to_string(),
                JsValue::String(id) => state.heap.string(*id).unwrap().to_utf8_lossy(),
                JsValue::BigInt(id) => state.heap.bigint(*id).unwrap().to_string(),
                JsValue::ShortBigInt(value) => value.to_string(),
                _ => panic!("unexpected leaf {output:?}"),
            };
            assert_eq!(actual, expected, "{source}");
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
            assert!(!runtime.0.deferred_references.has_pending());
        }
        drop(execution);
        runtime.collect_if_requested().unwrap();
        assert!(runtime.0.state.borrow().heap.object(cycle).is_err());
    }
}

#[test]
fn selected_native_named_getters_complete_without_leaf_pressure_service() {
    for (input, expected) in [
        ("Object.defineProperty({},'x',{get:Math.clz32})", 32.0),
        (
            "Object.defineProperty(new Date(42),'x',{get:Date.prototype.getTime})",
            42.0,
        ),
        (
            "Object.defineProperty(new Number(7),'x',{get:Number.prototype.valueOf})",
            7.0,
        ),
    ] {
        for (source, opcode) in [
            ("(function(o){return o.x})", Opcode::GetFieldCached),
            (
                "(function(o){const {x:v}=o;return v})",
                Opcode::GetField2Cached,
            ),
        ] {
            let runtime = Runtime::new();
            let mut context = runtime.new_context().unwrap();
            runtime.set_gc_policy(GcPolicy::Manual).unwrap();
            let cycle = unreachable_cycle(&runtime, &mut context);
            let input = context.eval(input).unwrap();
            let input = runtime.into_jsvalue(input).unwrap();
            let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
            push_input(&mut execution, id, input);
            let owners = std::rc::Rc::strong_count(&runtime.0);
            runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
            runtime.0.gc_pressure.remaining.set(0);
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
                VmAction::Complete
            ));
            assert_eq!(
                execution.pending.as_ref().unwrap().as_number(),
                Some(expected)
            );
            assert!(state.heap.object(cycle).is_ok());
            assert!(runtime.0.gc_pressure.requested());
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
            assert!(execution.selected_named_read.is_none());
            assert!(execution.selected_native_query.is_none());
            assert!(!runtime.0.deferred_references.has_pending());
        }
    }
}

#[test]
fn selected_native_named_boundary_resumes_its_original_receiver_and_callback_once() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(() => {
                    let trace = '';
                    const o = {
                        [Symbol.toPrimitive](hint) {
                            trace += hint;
                            Object.defineProperty(this, 'x', {
                                configurable: true,
                                get() { throw 'selected getter was replayed'; }
                            });
                            return 7;
                        },
                        toISOString() { trace += ':iso'; return 42; }
                    };
                    Object.defineProperty(o, 'x', {
                        configurable: true, get: Date.prototype.toJSON
                    });
                    const {x: result} = o;
                    return result === 42 && trace === 'number:iso';
                })()"#,
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(!runtime.is_poisoned());
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn selected_native_named_throw_restores_lower_operands_and_catch_window() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(() => {
                    const bad = Object.defineProperty({}, 'x', {get: Date.prototype.getTime});
                    function read() {
                        try { return 99 + bad.x; }
                        catch (error) { return error instanceof TypeError ? 7 : -1; }
                    }
                    return 35 + read();
                })()"#,
            )
            .unwrap(),
        Value::Int(42)
    );
    assert!(!runtime.is_poisoned());
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn selected_native_named_host_overflow_keeps_the_chosen_inputs_without_a_native_activation() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(input) = context
        .eval("Object.defineProperty({},'x',{get:Math.clz32})")
        .unwrap()
    else {
        panic!("native getter receiver")
    };
    let input_id = input.object_id();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.x})",
        Opcode::GetFieldCached,
    );
    push_input(&mut execution, id, JsValue::Object(input.into_handle()));
    let previous_top = runtime.0.host_stack_top.replace(Some(0));
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            VmAction::NativeProgress
        ));
        assert_eq!(runtime.0.active_frame_depth.get(), 1);
        assert!(execution.selected_native_query.is_some());
        assert!(execution.pending.is_none());
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 0);
        assert_eq!(state.heap.object_strong_count(input_id), Ok(1));
    }
    runtime.0.host_stack_top.set(previous_top);
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(input_id).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn all_object_autoinit_reads_publish_the_result_before_servicing_pressure() {
    for family in 0..8 {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        runtime.set_gc_policy(GcPolicy::Manual).unwrap();
        let cycle = unreachable_cycle(&runtime, &mut context);
        let object = runtime.new_object(None).unwrap();
        let (mut execution, id) = read_fixture(
            &runtime,
            &mut context,
            "(function(o){return o.lazyCycleFact})",
            Opcode::GetFieldCached,
        );
        let atom = {
            let frame = execution.frames.current_mut(id).unwrap();
            let decoded = frame
                .executable
                .exec
                .decode_published(frame.resume_pc as u32)
                .unwrap();
            frame.executable.property_key_atoms.as_ref().unwrap()[decoded.operand(0) as usize]
        };
        let realm = context.realm;
        let initializer = match family {
            0 => AutoInitProperty::FunctionPrototype { realm },
            1 => AutoInitProperty::NativeBuiltin {
                realm,
                target: crate::engine::builtins::native::NativeFunctionId::MathClz32,
                name: "named-cycle",
                length: 1,
                min_readable_args: 1,
            },
            2 => AutoInitProperty::ArrayUnscopables { realm },
            3 => AutoInitProperty::Math { realm },
            4 => AutoInitProperty::Reflect { realm },
            5 => AutoInitProperty::Json { realm },
            6 => AutoInitProperty::Atomics { realm },
            7 => AutoInitProperty::String {
                realm,
                value: "leaf",
            },
            _ => unreachable!(),
        };
        runtime
            .0
            .state
            .borrow_mut()
            .replace_layout_with_poison(
                &runtime.0.poisoned,
                object.object_id(),
                None,
                &[ShapeEntry {
                    atom: AtomIdx::from_raw(atom.raw()),
                    flags: PropertyFlags::data(true, false, true),
                }],
                vec![PropertySlot::auto_init(initializer)].into(),
            )
            .unwrap();
        push_input(&mut execution, id, JsValue::Object(object.into_handle()));
        runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
        runtime.0.gc_pressure.remaining.set(0);
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
                VmAction::Complete
            ));
            assert_eq!(state.heap.object(cycle).is_err(), family != 7);
            assert_eq!(runtime.0.gc_pressure.requested(), family == 7);
            match execution.pending.as_ref().unwrap() {
                JsValue::Object(output) => assert!(state.heap.object(*output).is_ok()),
                JsValue::String(output) if family == 7 => {
                    assert_eq!(state.heap.string(*output).unwrap().to_utf8_lossy(), "leaf")
                }
                _ => panic!("factory result"),
            }
            assert!(!runtime.0.deferred_references.has_pending());
        }
        drop(execution);
        runtime.run_gc().unwrap();
    }
}

#[test]
fn ordinary_getter_get_and_get2_share_the_existing_loop_and_receiver_roles() {
    for (object, source, opcode, expected) in [
        (
            "Object.defineProperty({tag:42},'x',{get:function(){return this.tag}})",
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
            42,
        ),
        (
            "Object.defineProperty({tag:42},'x',{get:function(){return function(){return this.tag}}})",
            "(function(o){return o.x()})",
            Opcode::GetField2Cached,
            42,
        ),
        (
            "(()=>{let captured=37;return Object.defineProperty({},'x',{get:function(){return captured}})})()",
            "(function(o){return o.x})",
            Opcode::GetFieldCached,
            37,
        ),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let input = context.eval(object).unwrap();
        let input = runtime.into_jsvalue(input).unwrap();
        let (mut execution, id) = read_fixture(&runtime, &mut context, source, opcode);
        push_input(&mut execution, id, input);
        let owners = std::rc::Rc::strong_count(&runtime.0);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert!(matches!(
            execute_frame(&runtime, &mut execution, id).unwrap(),
            VmAction::Complete
        ));
        assert_eq!(execution.pending, Some(JsValue::Int(expected)));
        assert_eq!(execution.frames.current_id(), Some(id));
        assert!(execution.selected_named_read.is_none());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(!runtime.0.deferred_references.has_pending());
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
            assert_eq!(events.get("core.pc_authentication"), Some(&1));
            assert_eq!(events.get("core.internal_named_getter"), Some(&1));
            assert!(!events.contains_key("driver_handoff.named_read"));
            assert!(!events.contains_key("core.legacy_boundary.selected_getter_root"));
            assert!(!events.contains_key("core.runtime_clone"));
        }
    }
}

#[test]
fn ordinary_getter_preserves_lower_operands_and_catch_handler_windows() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    for source in [
        "(()=>{let o={get x(){return 7}};return 11+o.x===18})()",
        "(()=>{let marker={},o={get x(){throw marker}};let log='';try{11+o.x}catch(e){if(e!==marker)return false;log+='c'}finally{log+='f'}return log==='cf' && 23+({get x(){return 19}}).x===42})()",
        "(()=>{let o={get x(){let inner={get y(){return 17}};return inner.y}};return [11,o.x,19].join(',')==='11,17,19'})()",
    ] {
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
    }
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn primitive_getter_this_uses_the_existing_strict_and_sloppy_binding_contract() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"(()=>{
        Object.defineProperty(Number.prototype,'strictNamedThis',{get:function(){'use strict';return this},configurable:true});
        Object.defineProperty(Number.prototype,'sloppyNamedThis',{get:function(){return this},configurable:true});
        return (23).strictNamedThis===23 && typeof (23).sloppyNamedThis==='object';
    })()"#).unwrap(),Value::Bool(true));
}

#[test]
fn getter_frame_and_parent_fault_pc_precede_resume_in_backtraces_and_throw() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let source = "let marker={};\nlet o=Object.defineProperty({},'x',{get:function namedGetter(){marker.stack=new Error().stack;throw marker}});\nfunction namedParent(){\n return 11+o.x;\n}\ntry{namedParent()}catch(e){e===marker && e.stack.includes('at namedGetter (named-get.js:2:') && e.stack.includes('at namedParent (named-get.js:4:')}";
    assert_eq!(
        context.eval_with_filename(source, "named-get.js").unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn getter_result_keeps_the_last_receiver_live_through_collection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(pair) = context.eval("(()=>{let o=Object.defineProperty({},'x',{get:function(){return this}});return [o,new WeakRef(o)]})()").unwrap()
    else { panic!("fixture pair") };
    let zero = runtime.intern_property_key("0").unwrap();
    let one = runtime.intern_property_key("1").unwrap();
    let Value::Object(receiver) = context.get_property(&pair, &zero).unwrap() else {
        panic!("receiver")
    };
    let Value::Object(weak) = context.get_property(&pair, &one).unwrap() else {
        panic!("WeakRef")
    };
    let receiver_id = receiver.object_id();
    drop(pair);
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.x})",
        Opcode::GetFieldCached,
    );
    push_input(&mut execution, id, JsValue::Object(receiver.into_handle()));
    assert!(matches!(
        execute_frame(&runtime, &mut execution, id).unwrap(),
        VmAction::Complete
    ));
    assert_eq!(execution.pending, Some(JsValue::Object(receiver_id)));
    runtime.run_gc().unwrap();
    assert!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .weak_ref_target(weak.object_id())
            .unwrap()
            .is_some()
    );
    drop(execution);
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().heap.object(receiver_id).is_err());
    assert!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .weak_ref_target(weak.object_id())
            .unwrap()
            .is_none()
    );
}

#[test]
fn ordinary_getter_authentication_overflow_preserves_the_original_window_and_fault() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(receiver) = context
        .eval("Object.defineProperty({},'x',{get:function(){throw 'must not run'}})")
        .unwrap()
    else {
        panic!("receiver")
    };
    let object = receiver.object_id();
    let key = runtime.intern_property_key("x").unwrap();
    let getter = {
        let state = runtime.0.state.borrow();
        let data = state.heap.object(object).unwrap();
        let index = state
            .heap
            .shape(data.shape)
            .unwrap()
            .find(AtomIdx::from_raw(key.atom().raw()))
            .unwrap() as usize;
        let PropertySlot::Accessor { get, .. } = &data.slots[index] else {
            panic!("getter slot")
        };
        get.option().unwrap()
    };
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.x})",
        Opcode::GetFieldCached,
    );
    push_input(&mut execution, id, JsValue::Object(receiver.into_handle()));
    let fault = execution.frames.current_mut(id).unwrap().resume_pc;
    let mut state = runtime.0.state.borrow_mut();
    let original = state.heap.object_strong_count(getter).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(getter), u32::MAX - 1);
    let result = execute_frame_in_state(&runtime, &mut state, &mut execution, id);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(getter), original);
    assert!(result.unwrap_err().message().contains("overflow"));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (fault, fault));
    assert_eq!(execution.slots.depth(&frame.window), 1);
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Object(object)
    );
    assert_eq!(state.heap.object_strong_count(object), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn completed_named_read_cleanup_failure_keeps_published_result_and_skips_gc() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let input = runtime
        .into_jsvalue(context.eval("({x:7})").unwrap())
        .unwrap();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.x})",
        Opcode::GetFieldCached,
    );
    push_input(&mut execution, id, input);
    let frame = execution.frames.current_mut(id).unwrap();
    let fault = frame.resume_pc;
    let decoded = frame
        .executable
        .exec
        .decode_published(fault as u32)
        .unwrap();
    let next = decoded.next_pc as usize;
    let atom =
        frame.executable.property_key_atoms.as_ref().unwrap()[decoded.operand_or_zero(0) as usize];
    let realm = frame.executable.realm;
    // Carry an actual canonical selection so this witnesses the new selected
    // completion, rather than the existing IC's pre-advance cleanup contract.
    let read = runtime
        .0
        .state
        .borrow_mut()
        .prepare_value_read_in_state(
            &runtime.0.poisoned,
            runtime.domain_id(),
            realm,
            execution.slots.peek(&frame.window, 0).unwrap(),
            atom,
            None,
        )
        .unwrap();
    let crate::engine::object::ReadStep::Ready(read) = read else {
        panic!("selected ordinary data")
    };
    execution.selected_named_read = Some(SelectedNamedRead::Prepared(read));
    let invalid = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    runtime.0.gc_pressure.remaining.set(0);
    let mut state = runtime.0.state.borrow_mut();
    for object in [invalid, later] {
        state
            .heap
            .queue_release_for_test(RawId::Object(object))
            .unwrap();
    }
    state
        .heap
        .set_strong_count_for_test(RawId::Object(invalid), 1);
    assert!(execute_frame_in_state(&runtime, &mut state, &mut execution, id).is_err());
    assert!(runtime.is_poisoned());
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (fault, next));
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Int(7)
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
}

#[test]
fn getter_selection_retirement_failure_stops_before_child_publication_or_suffix_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(input) = context
        .eval("Object.defineProperty({},'x',{get:function(){throw 'must not run'}})")
        .unwrap()
    else {
        panic!("getter receiver")
    };
    let input_id = input.object_id();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.x})",
        Opcode::GetFieldCached,
    );
    push_input(&mut execution, id, JsValue::Object(input.into_handle()));
    let fault = execution.frames.current_mut(id).unwrap().resume_pc;
    let invalid = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    for object in [invalid, later] {
        state
            .heap
            .queue_release_for_test(RawId::Object(object))
            .unwrap();
    }
    state
        .heap
        .set_strong_count_for_test(RawId::Object(invalid), 1);
    assert!(execute_frame_in_state(&runtime, &mut state, &mut execution, id).is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(execution.frames.current_id(), Some(id));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (fault, fault));
    assert_eq!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        &JsValue::Object(input_id)
    );
    // The selected this and preserved receiver suffix remain quarantined.
    assert_eq!(state.heap.object_strong_count(input_id), Ok(3));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(execution.pending.is_none());
}

#[test]
fn selected_proxy_boundary_owns_the_chosen_effect_and_no_property_replay() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context
        .eval("new Proxy({x:7},{get(t,k,r){return Reflect.get(t,k,r)}})")
        .unwrap()
    else {
        panic!("Proxy")
    };
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){return o.x})",
        Opcode::GetFieldCached,
    );
    push_input(&mut execution, id, JsValue::Object(proxy.into_handle()));
    let action = execute_frame(&runtime, &mut execution, id).unwrap();
    assert!(matches!(action, VmAction::GetField { .. }));
    assert!(matches!(
        &execution.selected_named_read,
        Some(SelectedNamedRead::Prepared(
            crate::engine::object::OwnedRead::Proxy { .. }
        ))
    ));
    let owners = std::rc::Rc::strong_count(&runtime.0);
    drop(execution);
    assert!(std::rc::Rc::strong_count(&runtime.0) < owners);
    assert!(!runtime.is_poisoned());
}

#[test]
fn selected_shared_backing_stays_live_after_the_original_frame_and_wrapper_retire() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(input) = context
        .eval("(()=>{let a=new Uint32Array(new SharedArrayBuffer(4));a[0]=42;return a})()")
        .unwrap()
    else {
        panic!("shared view")
    };
    let input_id = input.object_id();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o){const {'0':v}=o;return v})",
        Opcode::GetField2Cached,
    );
    push_input(&mut execution, id, JsValue::Object(input.into_handle()));
    assert!(matches!(
        execute_frame(&runtime, &mut execution, id).unwrap(),
        VmAction::GetField { .. }
    ));
    let Some(SelectedNamedRead::Shared(word)) = execution.selected_named_read.take() else {
        panic!("selected mutex seed")
    };
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(input_id).is_err());
    let word = word.read().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let read = state.own_typed_read_word(word).unwrap();
    assert!(matches!(
        &read,
        crate::engine::object::OwnedRead::Complete(Some(JsValue::Int(42)))
    ));
    read.retire(&mut state, &runtime.0.poisoned).unwrap();
    assert!(!runtime.is_poisoned());
}
