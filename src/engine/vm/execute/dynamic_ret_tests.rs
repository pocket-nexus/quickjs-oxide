//! Dynamic return addresses remain checked after continuous frame admission.
use crate::engine::{
    api::Runtime,
    code::{
        bytecode::Instruction,
        function::{UnlinkedFunction, metadata::FunctionMetadata},
    },
    heap::ContextId,
    value::JsValue,
    vm::{
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
        frame::FrameId,
    },
};

/// Publish a valid Gosub program, then inject its dynamic address at the
/// subroutine entry. Invalid address values never weaken publication checks.
fn at_dynamic_return(
    runtime: &Runtime,
    realm: ContextId,
    drop_address: bool,
    address: JsValue,
) -> (RunningExecution, FrameId, usize) {
    let mut code = vec![
        Instruction::PushI32(i32::MAX),
        Instruction::Gosub(4),
        Instruction::Return,
        Instruction::Nop,
    ];
    if drop_address {
        code.extend([
            Instruction::DropGosub,
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ]);
    } else {
        code.push(Instruction::Ret);
    }
    let bytecode = runtime
        .publish_unlinked_function(
            realm,
            UnlinkedFunction::fixture(
                code,
                Vec::new(),
                FunctionMetadata {
                    max_stack: 2,
                    ..Default::default()
                },
            ),
        )
        .unwrap();
    let callable = runtime.new_bytecode_closure(realm, &bytecode).unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("published bytecode callable")
    };
    let entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        realm,
        &callable,
        JsValue::Undefined,
        JsValue::Undefined,
        Vec::new(),
        bytecode,
        closure_slots,
    )
    .unwrap();
    let pc = entry.executable.exec.exec_pc(4).unwrap() as usize;
    // The first instruction is wide. Its second word is a valid array index
    // but cannot be admitted as an instruction boundary by dynamic Ret.
    assert!(entry.executable.exec.opcode_at_exec(1).is_none());
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    let frame = execution.frames.current_mut(id).unwrap();
    execution
        .slots
        .push(&mut frame.window, JsValue::Int(42))
        .unwrap();
    execution.slots.push(&mut frame.window, address).unwrap();
    frame.resume_pc = pc;
    (execution, id, pc)
}

#[test]
fn dynamic_ret_rejects_negative_middle_and_out_of_range_targets_before_consumption() {
    for (target, message) in [
        (-1, "invalid ret value"),
        (1, "ret target is not an instruction boundary"),
        (i32::MAX, "ret target is not an instruction boundary"),
    ] {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let (mut execution, id, pc) =
            at_dynamic_return(&runtime, context.realm, false, JsValue::Int(target));
        assert!(
            matches!(super::execute_frame(&runtime, &mut execution, id), Err(ref error)
            if error.message() == message)
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Int(target)
        );
        assert_eq!(
            execution.slots.peek(&frame.window, 1).unwrap(),
            &JsValue::Int(42)
        );
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn dynamic_ret_and_gosub_cleanup_preserve_noninteger_heap_owner_for_unwind() {
    for (drop_address, message) in [
        (false, "invalid ret value"),
        (true, "invalid gosub cleanup value"),
    ] {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let address = runtime.new_object(None).unwrap().into_handle();
        let (mut execution, id, pc) = at_dynamic_return(
            &runtime,
            context.realm,
            drop_address,
            JsValue::Object(address),
        );
        assert!(
            matches!(super::execute_frame(&runtime, &mut execution, id), Err(ref error)
            if error.message() == message)
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Object(address)
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(address)
                .unwrap(),
            1
        );
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(address).is_err());
        assert!(!runtime.is_poisoned());
    }
}
