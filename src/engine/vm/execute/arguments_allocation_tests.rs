//! Actual Arguments/rest results and OP_apply requests stay owned through
//! collection, recoverable publication rejection and terminal cleanup.
use crate::engine::{
    api::{Context, Runtime, Value},
    atom::AtomIdx,
    code::bytecode::{ApplyKind, ArgumentsKind, Instruction},
    heap::{ObjectPayload, PropertySlot, RawId, RawValue},
    object::CallableRef,
    value::JsValue,
    vm::{
        bindings::FrameBinding,
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
        frame::FrameId,
        frames::ActiveFrameToken,
    },
};

fn execution_at(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    arguments: Vec<JsValue>,
    selected: impl Fn(&Instruction) -> bool,
) -> (RunningExecution, FrameId, usize) {
    let Value::Object(function) = context.eval(source).unwrap() else {
        panic!("function fixture")
    };
    let callable = CallableRef::from_validated_object(function);
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("bytecode fixture")
    };
    let mut entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        context.realm,
        &callable,
        JsValue::Undefined,
        JsValue::Undefined,
        arguments,
        bytecode,
        closure_slots,
    )
    .unwrap();
    let pc = entry
        .executable
        .exec
        .test_ir()
        .iter()
        .position(selected)
        .unwrap();
    let pc = entry.executable.exec.exec_pc(pc as u32).unwrap() as usize;
    entry
        .cold
        .entry_guard
        .take()
        .unwrap()
        .finish(&mut runtime.0.state.borrow_mut())
        .unwrap();
    entry.active_frame = ActiveFrameToken::unmaterialized();
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    execution.frames.current_mut(id).unwrap().resume_pc = pc;
    (execution, id, pc)
}
fn clean(runtime: &Runtime) {
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn resident_arguments_and_rest_publish_exact_actual_values_before_pressure_collection() {
    for (source, kind) in [
        (
            "(function(a,b){return arguments})",
            Some(ArgumentsKind::Mapped),
        ),
        (
            "(function(a,b){'use strict';return arguments})",
            Some(ArgumentsKind::Unmapped),
        ),
        ("(function(a,...r){return r})", None),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(garbage) = context
            .eval("(()=>{let g={};g.self=g;return g})()")
            .unwrap()
        else {
            panic!()
        };
        let garbage = garbage.into_handle();
        runtime.release_jsvalue(JsValue::Object(garbage)).unwrap();
        let marker = runtime.new_object(None).unwrap().into_handle();
        let atom = runtime.new_symbol(None).unwrap().into_atom();
        let symbol = AtomIdx::from_raw(atom.raw());
        let (mut execution, id, pc) = execution_at(
            &runtime,
            &mut context,
            source,
            vec![
                JsValue::Object(marker),
                JsValue::Int(4),
                JsValue::Symbol(symbol),
            ],
            |op| match kind {
                Some(kind) => matches!(op, Instruction::Arguments(found) if *found == kind),
                None => matches!(op, Instruction::Rest(1)),
            },
        );
        let owners = std::rc::Rc::strong_count(&runtime.0);
        runtime.0.gc_pressure.remaining.set(1);
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        {
            let mut state = runtime.0.state.borrow_mut();
            assert!(matches!(
                super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
                super::VmAction::Complete
            ));
            let Some(JsValue::Object(result)) = execution.pending.as_ref() else {
                panic!("resident result")
            };
            let result = state.heap.object(*result).unwrap();
            if let Some(kind) = kind {
                let ObjectPayload::Arguments { mapped, .. } = &result.payload else {
                    panic!("Arguments result")
                };
                assert_eq!(*mapped, kind == ArgumentsKind::Mapped);
                let shape = state.heap.shape(result.shape).unwrap();
                for (index, expected) in [
                    RawValue::Object(marker),
                    RawValue::Int(4),
                    RawValue::Symbol(symbol),
                ]
                .iter()
                .enumerate()
                {
                    let atom =
                        crate::engine::atom::Atom::from_immediate_integer(index as u32).unwrap();
                    let slot =
                        &result.slots[shape.find(AtomIdx::from_raw(atom.raw())).unwrap() as usize];
                    let actual = match slot {
                        PropertySlot::Data(value) => value,
                        PropertySlot::VarRef(cell) => &state.heap.var_ref(*cell).unwrap().value,
                        _ => panic!("actual indexed value"),
                    };
                    assert!(match (actual, expected) {
                        (RawValue::Object(actual), RawValue::Object(expected)) =>
                            actual == expected,
                        (RawValue::Int(actual), RawValue::Int(expected)) => actual == expected,
                        (RawValue::Symbol(actual), RawValue::Symbol(expected)) =>
                            actual == expected,
                        _ => false,
                    });
                }
            } else {
                let ObjectPayload::Array {
                    dense: Some(values),
                } = &result.payload
                else {
                    panic!("rest Array")
                };
                assert!(
                    matches!(values.as_slice(), [RawValue::Int(4), RawValue::Symbol(actual)] if *actual == symbol)
                );
            }
            assert!(state.heap.object(garbage).is_err());
            assert!(runtime.0.gc_pressure.remaining.get() > 0);
            assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
            let frame = execution.frames.current_mut(id).unwrap();
            assert!(frame.active_frame.is_materialized());
            assert!(frame.resume_pc > pc);
            assert!(!runtime.0.deferred_references.has_pending());
        }
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(
                events.get(if kind.is_some() {
                    "core.internal_arguments"
                } else {
                    "core.internal_rest"
                }),
                Some(&1)
            );
            assert_eq!(events.get("execute.action.arguments"), None);
            assert_eq!(events.get("execute.action.rest"), None);
            assert_eq!(events.get("runtime.clone"), None);
        }
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(marker).is_err());
        clean(&runtime);
    }
}

#[test]
fn resident_mapped_arguments_failed_commit_keeps_published_cell_and_original_fault() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id, pc) = execution_at(
        &runtime,
        &mut context,
        "(function(a){return arguments})",
        vec![JsValue::Object(marker)],
        |op| matches!(op, Instruction::Arguments(ArgumentsKind::Mapped)),
    );
    let frame = execution.frames.current_mut(id).unwrap();
    execution
        .slots
        .push(&mut frame.window, JsValue::Int(9))
        .unwrap();
    runtime.0.gc_pressure.remaining.set(1);
    {
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.counts();
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert!(
            error
                .message()
                .contains("owned operand stack exceeds verified capacity")
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Int(9)
        );
        let FrameBinding::Captured(cell) = execution.slots.parameter(&frame.window, 0).unwrap()
        else {
            panic!("published parameter capture")
        };
        assert_eq!(state.heap.var_ref_strong_count(*cell), Ok(1));
        assert!(
            matches!(state.heap.var_ref(*cell).unwrap().value, RawValue::Object(actual) if actual == marker)
        );
        // Original argv and the captured formal each own one marker edge.
        assert_eq!(state.heap.object_strong_count(marker), Ok(2));
        let after = state.heap.counts();
        assert_eq!(after.object_nodes, before.object_nodes);
        assert_eq!(after.var_ref_nodes, before.var_ref_nodes + 1);
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    clean(&runtime);
}

#[test]
fn resident_arguments_partial_capture_failure_reuses_existing_cells_without_duplication() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id, pc) = execution_at(
        &runtime,
        &mut context,
        "(function(a,b){return arguments})",
        vec![JsValue::Object(marker), JsValue::Int(4)],
        |op| matches!(op, Instruction::Arguments(ArgumentsKind::Mapped)),
    );
    let mut state = runtime.0.state.borrow_mut();
    let metadata = crate::engine::code::function::metadata::ClosureVariable {
        source: crate::engine::code::function::metadata::ClosureSource::ParentArgument(1),
        name: crate::engine::code::function::metadata::ClosureVariableName::None,
        is_lexical: false,
        is_const: false,
        kind: crate::engine::code::function::metadata::ClosureVariableKind::Normal,
    };
    let blocked = {
        let mut segment =
            crate::engine::vm::stack::FrameExecution::admit(&mut execution, id).unwrap();
        let mut turn = segment.frame();
        crate::engine::vm::bindings::capture::capture_frame_binding(
            &mut state,
            &runtime.0.poisoned,
            turn.transaction.slots().parameter_mut(1).unwrap(),
            metadata,
        )
        .unwrap()
    };
    state.release_var_ref_handle(blocked).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::VarRef(blocked), u32::MAX);
    let failure = super::execute_frame_in_state(&runtime, &mut state, &mut execution, id);
    let frame = execution.frames.current_mut(id).unwrap();
    let FrameBinding::Captured(first) = execution.slots.parameter(&frame.window, 0).unwrap() else {
        panic!("first capture survives")
    };
    let first = *first;
    state
        .heap
        .set_strong_count_for_test(RawId::VarRef(blocked), 1);
    assert!(failure.is_err());
    assert_eq!(state.heap.var_ref_strong_count(first), Ok(1));
    // Original argv and the captured formal each own one marker edge.
    assert_eq!(state.heap.object_strong_count(marker), Ok(2));
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert!(!runtime.is_poisoned());
    assert!(matches!(
        super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
        super::VmAction::Complete
    ));
    let frame = execution.frames.current_mut(id).unwrap();
    assert!(
        matches!(execution.slots.parameter(&frame.window, 0).unwrap(), FrameBinding::Captured(actual) if *actual == first)
    );
    assert_eq!(state.heap.var_ref_strong_count(first), Ok(2));
    drop(state);
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_apply_immediate_native_completion_does_not_reserve_obsolete_carrier_identity() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let native = runtime
        .into_jsvalue(context.eval("Math.abs").unwrap())
        .unwrap();
    let (mut execution, id, pc) = execution_at(
        &runtime,
        &mut context,
        "(function(f){return f(...[])})",
        Vec::new(),
        |op| matches!(op, Instruction::Apply(ApplyKind::Call)),
    );
    let frame = execution.frames.current_mut(id).unwrap();
    frame.property_generation = u64::MAX;
    for value in [native, JsValue::Undefined, JsValue::Null] {
        execution.slots.push(&mut frame.window, value).unwrap();
    }
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        assert!(
            matches!(execution.pending.as_ref(), Some(JsValue::Float(value)) if value.is_nan())
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.property_generation, u64::MAX);
        assert!(frame.resume_pc > pc);
        assert_eq!(execution.slots.depth(&frame.window), 0);
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_apply_real_callback_identity_rejection_retires_armed_prefix_without_effect() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let _ = context.eval("globalThis.callbackHits=0").unwrap();
    let callee = runtime
        .into_jsvalue(
            context
                .eval("(function(){callbackHits++;return 42})")
                .unwrap(),
        )
        .unwrap();
    let receiver = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id, pc) = execution_at(
        &runtime,
        &mut context,
        "(function(f){return f(...[])})",
        Vec::new(),
        |op| matches!(op, Instruction::Apply(ApplyKind::Call)),
    );
    let frame = execution.frames.current_mut(id).unwrap();
    frame.property_generation = u64::MAX;
    for value in [callee, JsValue::Object(receiver), JsValue::Null] {
        execution.slots.push(&mut frame.window, value).unwrap();
    }
    {
        let mut state = runtime.0.state.borrow_mut();
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert_eq!(error.message(), "property operation identity exhausted");
        assert!(state.heap.object(receiver).is_err());
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(frame.property_generation, u64::MAX);
        assert_eq!(execution.slots.depth(&frame.window), 0);
        assert!(execution.selected_native_query.is_none());
        assert!(execution.pending.is_none());
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    assert_eq!(context.eval("callbackHits").unwrap(), Value::Int(0));
    clean(&runtime);
}
