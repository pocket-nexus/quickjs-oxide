use crate::engine::{
    api::Runtime,
    code::{
        bytecode::Instruction as I,
        function::metadata::{ClosureVariableKind, VariableDefinition},
        runtime::PublishedFunctionSnapshot,
    },
    value::JsValue,
    vm::{
        CallInput,
        bindings::FrameBinding,
        execution::{ExecutionLimits, RunningExecution},
        frame::{ColdFrame, FrameCold, FrameEntry, FrameId},
        frames::ActiveFrameToken,
        stack::FrameStorage,
    },
};

fn fixture(
    runtime: &Runtime,
    realm: crate::engine::heap::ContextId,
    code: Vec<I>,
    capacity: u16,
) -> (RunningExecution, FrameId) {
    let function = runtime.new_object(None).unwrap();
    let mut executable = PublishedFunctionSnapshot::empty_for_test(realm);
    executable.metadata.max_stack = capacity;
    executable.metadata.local_count = 1;
    executable.metadata.argument_count = 1;
    let definition = VariableDefinition {
        name: None,
        is_lexical: false,
        is_const: false,
        is_parameter_initializer: false,
        kind: ClosureVariableKind::Normal,
    };
    executable.local_definitions = vec![definition].into();
    executable.argument_definitions = vec![definition].into();
    executable.fusion = crate::engine::code::fusion::FusionPlan::build(&code, &[definition]);
    executable.code = code.into();
    let entry = FrameEntry {
        property_generation: 0,
        iterator_generation: 0,
        caller_realm: realm,
        active_frame: ActiveFrameToken(0),
        initialize_bindings: false,
        executable,
        cold: ColdFrame::new(FrameCold {
            rare: std::cell::OnceCell::new(),
            return_to: None,
            entry_guard: None,
            input: CallInput::new(
                runtime,
                JsValue::Undefined,
                JsValue::Undefined,
                Some(function.clone()),
            )
            .into(),
            function: function.into(),
            closure_slots: Default::default(),
            reusable_captured_locals: vec![false],
        }),
        storage: FrameStorage {
            original_arguments: vec![],
            parameters: vec![FrameBinding::Direct(JsValue::Int(7))],
            locals: vec![FrameBinding::Direct(JsValue::Int(0))],
            operands: vec![],
        },
    };
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(&mut execution, entry).unwrap();
    (execution, id)
}

// Both modes execute the real resident match over separate runtime/frame state.
// Reading every logical operand afterwards detects an un-restored backing hole.
fn observation<const CACHE: bool>(
    code: Vec<I>,
    capacity: u16,
) -> (String, Vec<String>, String, usize, usize) {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let (mut execution, id) = fixture(&runtime, context.realm, code, capacity);
    let result = super::super::run_impl::<CACHE>(&mut execution, id);
    let result = match result {
        Ok(exit) => format!("{exit:?}"),
        Err(error) => format!("{error}"),
    };
    let frame = execution.frames.current_mut(id).unwrap();
    let depth = execution.slots.depth(&frame.window);
    let operands = (0..depth)
        .map(|index| format!("{:?}", execution.slots.peek(&frame.window, index).unwrap()))
        .collect();
    let pending = format!("{:?}", execution.pending);
    (result, operands, pending, frame.fault_pc, frame.resume_pc)
}

#[test]
fn scalar_tos_run_matches_canonical_arithmetic_fusion_and_mixed_handlers() {
    for code in [
        vec![
            I::PushI32(40),
            I::PushI32(2),
            I::Add,
            I::SetLocal(0),
            I::Drop,
            I::GetLocal(0),
            I::Return,
        ],
        vec![
            I::PushI32(41),
            I::PutArg(0),
            I::GetArg(0),
            I::Inc,
            I::Return,
        ],
        vec![
            I::PushI32(3),
            I::PushI32(2),
            I::Lt,
            I::IfTrue(6),
            I::PushI32(42),
            I::Return,
            I::PushI32(9),
            I::Return,
        ],
        vec![
            I::GetLocal(0),
            I::PostInc,
            I::PutLocal(0),
            I::Drop,
            I::GetLocal(0),
            I::Return,
        ],
        vec![I::PushI32(3), I::Dup, I::Mul, I::Not, I::Return],
        vec![I::PushI32(2), I::PushI32(3), I::Swap, I::Sub, I::Return],
    ] {
        assert_eq!(
            observation::<false>(code.clone(), 4),
            observation::<true>(code, 4)
        );
    }
}

#[test]
fn scalar_tos_run_restores_before_call_suspend_throw_and_cold_exits() {
    for final_instruction in [
        I::Call(0),
        I::Await,
        I::Yield,
        I::Throw,
        I::Catch(0),
        I::Arguments(crate::engine::code::bytecode::ArgumentsKind::Mapped),
        I::ReturnUndefined,
    ] {
        let code = vec![I::PushI32(42), final_instruction];
        let canonical = observation::<false>(code.clone(), 2);
        let cached = observation::<true>(code, 2);
        assert_eq!(canonical, cached);
        assert_eq!(
            cached.1,
            vec!["JsValue::Int(42)"],
            "an exit cannot hide a scalar owner from the driver"
        );
    }
}

#[test]
fn scalar_tos_run_capacity_error_restores_committed_prefix_and_both_pcs() {
    let code = vec![I::PushI32(41), I::PostInc, I::Return];
    let canonical = observation::<false>(code.clone(), 1);
    let cached = observation::<true>(code, 1);
    assert_eq!(canonical, cached);
    assert_eq!(cached.1, vec!["JsValue::Int(41)"]);
    assert_eq!((cached.3, cached.4), (1, 1));
}

#[test]
fn scalar_tos_materialize_restores_both_operands_at_the_original_pc() {
    let code = vec![I::PushI32(42), I::PushI32(0), I::GetArrayEl2];
    let canonical = observation::<false>(code.clone(), 2);
    let cached = observation::<true>(code, 2);
    assert_eq!(canonical, cached);
    assert_eq!(cached.0, "Materialize");
    assert_eq!(cached.1, vec!["JsValue::Int(0)", "JsValue::Int(42)"]);
    assert_eq!((cached.3, cached.4), (2, 2));
}

#[test]
fn scalar_tos_unwind_restores_committed_stack_and_program_counter_together() {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let (mut execution, id) = fixture(&runtime, context.realm, vec![I::ReturnUndefined], 2);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let frame = execution.frames.current_mut(id).unwrap();
        // Match the declaration/drop order in run_with_modes. A live Runtime
        // borrow also prevents restoration from accidentally calling the heap.
        let _borrow = runtime.0.state.borrow_mut();
        let mut transaction = execution
            .slots
            .frame_transaction_with_scalar_tos(&mut frame.cold.window)
            .unwrap();
        let mut pc = super::super::ProgramCounter::new(&mut frame.fault_pc, &mut frame.resume_pc);
        transaction.slots().push(JsValue::Int(41)).unwrap();
        transaction.slots().push(JsValue::Int(42)).unwrap();
        pc.fault = 7;
        pc.resume = 8;
        panic!("combined stack/PC unwind probe");
    }));
    assert!(result.is_err());
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!((frame.fault_pc, frame.resume_pc), (7, 8));
    assert_eq!(execution.slots.depth(&frame.window), 2);
    assert!(matches!(
        execution.slots.peek(&frame.window, 0).unwrap(),
        JsValue::Int(42)
    ));
    assert!(matches!(
        execution.slots.peek(&frame.window, 1).unwrap(),
        JsValue::Int(41)
    ));
}

#[cfg(feature = "profiling")]
#[test]
fn scalar_tos_run_really_retains_cache_between_short_facades() {
    use crate::engine::api::profiling::CostProfile;
    let profile = CostProfile::start();
    let result = observation::<true>(
        vec![
            I::PushI32(40),
            I::PushI32(2),
            I::Add,
            I::PutLocal(0),
            I::GetLocal(0),
            I::Return,
        ],
        2,
    );
    assert_eq!(result.2, "Some(JsValue::Int(42))");
    let costs = profile.snapshot();
    for event in ["tos.commit", "tos.hit"] {
        assert!(
            costs
                .owned_execution_events
                .get(event)
                .copied()
                .unwrap_or(0)
                > 0,
            "{event}"
        );
    }
}
