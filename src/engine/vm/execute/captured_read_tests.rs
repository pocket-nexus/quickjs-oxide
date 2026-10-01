//! Captured scalar reads keep the frame's cell owner and ordinary miss boundary.
use crate::engine::{
    api::{Runtime, Value},
    heap::{RawId, VarRefId, roots::VarRefView},
    value::JsValue,
    vm::{bindings::try_read_captured_immediate, execution::ExecutionLimits},
};

fn captured_frame(
    runtime: &Runtime,
    context: &mut crate::engine::api::Context,
    checked: bool,
) -> (super::RunningExecution, super::FrameId, usize, VarRefId) {
    let source = if checked {
        "(()=>{let x=7;return ()=>x;})()"
    } else {
        "(()=>{var x=7;return ()=>x;})()"
    };
    let Value::Object(function) = context.eval(source).unwrap() else {
        panic!("function");
    };
    let callable = runtime.as_callable(&function).unwrap().unwrap();
    let crate::engine::vm::call::CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("bytecode");
    };
    let cell = closure_slots.get(0).unwrap().id();
    let mut entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        context.realm,
        &callable,
        JsValue::Undefined,
        JsValue::Undefined,
        Vec::new(),
        bytecode,
        closure_slots,
    )
    .unwrap();
    drop(entry.cold.entry_guard.take());
    entry.active_frame = crate::engine::vm::frames::ActiveFrameToken::unmaterialized();
    let opcode = if checked {
        super::Opcode::GetVarRefCheck
    } else {
        super::Opcode::GetVarRef
    };
    let source_pc = (0..entry.executable.exec.instruction_len())
        .find(|&pc| entry.executable.exec.opcode_at_source(pc) == Some(opcode))
        .unwrap();
    let pc = entry.executable.exec.exec_pc(source_pc as u32).unwrap() as usize;
    let mut execution = super::RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(&mut execution, entry).unwrap();
    execution.frames.current_mut(id).unwrap().resume_pc = pc;
    (execution, id, pc, cell)
}

#[test]
fn captured_scalar_reads_complete_without_an_activation_or_cell_owner_copy() {
    for checked in [false, true] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id, _, cell) = captured_frame(&runtime, &mut context, checked);
        let count = runtime
            .0
            .state
            .borrow()
            .heap
            .var_ref_strong_count(cell)
            .unwrap();
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert!(matches!(
            super::execute_frame(&mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        assert_eq!(execution.pending.take(), Some(JsValue::Int(7)));
        assert!(
            !execution
                .frames
                .current_mut(id)
                .unwrap()
                .active_frame
                .is_materialized()
        );
        assert_eq!(
            runtime.0.state.borrow().heap.var_ref_strong_count(cell),
            Ok(count)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
        #[cfg(feature = "profiling")]
        assert_eq!(
            profile
                .snapshot()
                .execution_sites
                .iter()
                .filter(|(site, _)| site.kind == "captured_read")
                .map(|(_, cost)| cost.hits)
                .sum::<u64>(),
            1
        );
    }
}

#[test]
fn captured_heap_reads_keep_the_original_driver_and_result_owner() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let (mut execution, id, pc, cell) = captured_frame(&runtime, &mut context, true);
    let object = runtime.new_object(None).unwrap();
    let object_id = object.object_id();
    runtime
        .write_var_ref(
            &VarRefView::from_frame(&runtime, cell),
            JsValue::Object(object.into_handle()),
        )
        .unwrap();
    let action = super::execute_frame(&mut execution, id).unwrap();
    let super::VmAction::Binding {
        source,
        index,
        write,
        checked,
        keep,
    } = action
    else {
        panic!("heap result must keep its owning driver");
    };
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert_eq!(execution.slots.depth(&frame.window), 0);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(object_id),
        Ok(1)
    );
    execution.frames.materialize(&runtime).unwrap();
    crate::engine::vm::frame_operations::binding(
        &runtime,
        &mut execution,
        id,
        source,
        index,
        write,
        checked,
        keep,
    )
    .unwrap();
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(object_id),
        Ok(2)
    );
    assert!(matches!(
        super::execute_frame(&mut execution, id).unwrap(),
        super::VmAction::Complete
    ));
    let result = execution.pending.take().unwrap();
    assert_eq!(result, JsValue::Object(object_id));
    runtime.release_jsvalue(result).unwrap();
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(object_id),
        Ok(1)
    );
}

#[test]
fn captured_scalar_reads_decline_cleanup_and_leave_the_fault_checkpoint_unchanged() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let (mut execution, id, pc, cell) = captured_frame(&runtime, &mut context, false);
    let count = runtime
        .0
        .state
        .borrow()
        .heap
        .var_ref_strong_count(cell)
        .unwrap();
    let deferred = runtime.new_object(None).unwrap();
    {
        let _state = runtime.0.state.borrow();
        drop(deferred);
    }
    assert!(runtime.0.deferred_references.has_pending());
    assert!(matches!(
        super::execute_frame(&mut execution, id).unwrap(),
        super::VmAction::Binding {
            write: false,
            checked: false,
            ..
        }
    ));
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
    assert_eq!(execution.slots.depth(&frame.window), 0);
    assert_eq!(
        runtime.0.state.borrow().heap.var_ref_strong_count(cell),
        Ok(count)
    );
    assert!(runtime.0.deferred_references.has_pending());
    runtime.drain_deferred_references().unwrap();
    assert!(matches!(
        super::execute_frame(&mut execution, id).unwrap(),
        super::VmAction::Complete
    ));
    assert_eq!(execution.pending.take(), Some(JsValue::Int(7)));
}

#[test]
fn captured_scalar_admission_preserves_storage_permission_saturation_and_sentinels() {
    use crate::engine::code::function::metadata::ClosureVariableKind;
    let runtime = Runtime::new();
    let root = runtime
        .new_var_ref(JsValue::Int(1), false, false, ClosureVariableKind::Normal)
        .unwrap();
    assert_eq!(
        try_read_captured_immediate(&runtime, &root),
        Some(JsValue::Int(1))
    );
    let foreign = Runtime::new();
    assert_eq!(try_read_captured_immediate(&foreign, &root), None);
    {
        let _state = runtime.0.state.borrow();
        assert_eq!(try_read_captured_immediate(&runtime, &root), None);
    }
    for value in [
        JsValue::Undefined,
        JsValue::Null,
        JsValue::Bool(false),
        JsValue::Int(-3),
        JsValue::Float(-0.0),
        JsValue::ShortBigInt(4),
    ] {
        let raw = value.as_raw();
        runtime.write_var_ref(&root, value).unwrap();
        let value = try_read_captured_immediate(&runtime, &root).unwrap();
        match (&raw, &value) {
            (crate::engine::heap::RawValue::Float(expected), JsValue::Float(actual)) => {
                assert_eq!(expected.to_bits(), actual.to_bits());
            }
            _ => assert_eq!(JsValue::from_raw(raw), Some(value)),
        }
    }
    for count in [u32::MAX - 1, u32::MAX] {
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::VarRef(root.id()), count);
        assert_eq!(try_read_captured_immediate(&runtime, &root), None);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .var_ref_strong_count(root.id()),
            Ok(count)
        );
    }
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::VarRef(root.id()), 1);
    runtime.reset_var_ref_uninitialized(&root).unwrap();
    assert_eq!(try_read_captured_immediate(&runtime, &root), None);
    let private = runtime
        .new_uninitialized_captured_var_ref(true, true, ClosureVariableKind::PrivateField)
        .unwrap();
    assert_eq!(try_read_captured_immediate(&runtime, &private), None);
}
