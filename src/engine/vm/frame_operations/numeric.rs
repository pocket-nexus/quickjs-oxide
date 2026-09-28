//! One numeric operand transfer shared by same-frame and outer-driver paths.
use crate::engine::api::{Error, runtime::Runtime};
use crate::engine::vm::{
    driver::CallStep,
    execute::FallthroughPc,
    execution::RunningExecution,
    frame::FrameId,
    numeric::operation::{NumericKind, NumericStep},
};

pub(in crate::engine::vm) enum NumericProgress {
    Completed,
    Deferred(CallStep),
}
impl NumericProgress {
    pub(in crate::engine::vm) fn into_call_step(self) -> CallStep {
        match self {
            Self::Completed => CallStep::Entered,
            Self::Deferred(step) => step,
        }
    }
}

/// Commit in the original previous/value order. Pending owners remain outside
/// FrameSlots even if authentication or a later push fails.
pub(in crate::engine::vm) fn commit_output(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    value: crate::engine::value::JsValue,
    previous: Option<crate::engine::value::JsValue>,
    _depth: usize,
) -> Result<(), Error> {
    let mut value = Some(value);
    let mut previous = previous;
    let result = (|| {
        let frame = execution.frames.current_mut(id)?;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event(
            "numeric_legacy_fallthrough_recovery_decode",
        );
        let resume_pc = frame.next_pc()?;
        {
            let mut slots = execution.slots.borrow_frame_slots(&mut frame.window)?;
            if previous.is_some() {
                slots.push_pending(&mut previous)?;
            }
            slots.push_pending(&mut value)?;
        }
        frame.resume_pc = resume_pc;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_instruction(_depth);
        Ok(())
    })();
    if let Some(value) = value {
        let _ = runtime.release_jsvalue(value);
    }
    if let Some(value) = previous {
        let _ = runtime.release_jsvalue(value);
    }
    result
}

pub(in crate::engine::vm) fn try_complete_primitive(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: NumericKind,
    fallthrough: FallthroughPc,
) -> Result<Option<NumericProgress>, Error> {
    use crate::engine::value::JsValue;
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let depth = execution.slots.depth(&frame.window);
    let resume_pc = fallthrough.index();
    let mut transaction = execution.slots.frame_transaction(&mut frame.window)?;
    let (left, right) = {
        let mut slots = transaction.slots();
        // A malformed stack declines untouched: the canonical outer entry must
        // still pop RHS before reporting a missing LHS.
        for offset in 0..if kind.unary() { 1 } else { 2 } {
            if matches!(slots.peek(offset), Err(_) | Ok(JsValue::Object(_))) {
                return Ok(None);
            }
        }
        let right = slots.pop().expect("validated primitive operand");
        if kind.unary() {
            (right, None)
        } else {
            (
                slots.pop().expect("validated primitive left operand"),
                Some(right),
            )
        }
    };
    if !kind.primitive_arithmetic() {
        return match NumericStep::start(runtime, kind, left, right) {
            Ok(step) => crate::engine::vm::proxy_get_driver::start_numeric(
                runtime, execution, id, step, depth,
            )
            .map(Some),
            Err(error) => crate::engine::vm::property_driver::throw_error(runtime, realm, error)
                .map(|step| Some(NumericProgress::Deferred(step))),
        };
    }
    let output =
        match crate::engine::vm::numeric::operation::primitive_output(runtime, kind, left, right) {
            Ok(output) => output,
            Err(error) => {
                return crate::engine::vm::property_driver::throw_error(runtime, realm, error)
                    .map(|step| Some(NumericProgress::Deferred(step)));
            }
        };
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_execution_event("numeric_completed_without_query");
    let mut value = Some(output.value);
    let mut previous = output.previous;
    let result: Result<usize, Error> = (|| {
        {
            let mut slots = transaction.slots();
            if previous.is_some() {
                slots.push_pending(&mut previous)?;
            }
            slots.push_pending(&mut value)?;
        }
        Ok(resume_pc)
    })();
    if let Some(value) = value {
        let _ = runtime.release_jsvalue(value);
    }
    if let Some(value) = previous {
        let _ = runtime.release_jsvalue(value);
    }
    frame.resume_pc = result?;
    #[cfg(feature = "profiling")]
    crate::engine::api::profiling::record_owned_instruction(depth);
    Ok(Some(NumericProgress::Completed))
}

pub(in crate::engine::vm) fn complete(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    id: FrameId,
    kind: NumericKind,
) -> Result<NumericProgress, Error> {
    let frame = execution.frames.current_mut(id)?;
    let realm = frame.executable.realm;
    let depth = execution.slots.depth(&frame.window);
    // Preserve the existing unary/right-before-left pop order and error path.
    let (left, right) = if kind.unary() {
        (execution.slots.pop(&mut frame.window)?, None)
    } else {
        let right = execution.slots.pop(&mut frame.window)?;
        match execution.slots.pop(&mut frame.window) {
            Ok(left) => (left, Some(right)),
            Err(error) => {
                runtime
                    .release_jsvalue(right)
                    .map_err(crate::engine::vm::exception::runtime_error_to_vm_error)?;
                return Err(error);
            }
        }
    };
    let result = match NumericStep::start(runtime, kind, left, right) {
        Ok(step) => {
            crate::engine::vm::proxy_get_driver::start_numeric(runtime, execution, id, step, depth)?
        }
        Err(error) => NumericProgress::Deferred(crate::engine::vm::property_driver::throw_error(
            runtime, realm, error,
        )?),
    };
    match result {
        NumericProgress::Deferred(CallStep::Bridge) => {
            Err(Error::internal("numeric operation attempted replay"))
        }
        result => Ok(result),
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::api::{Runtime, Value};
    use crate::engine::value::JsValue;

    use super::*;
    use crate::engine::vm::{
        call::CallableExecution,
        conversion_driver::{PrimitiveCompletion, complete_primitives},
        execute::{VmAction, execute_frame},
        execution::ExecutionLimits,
        frame::{ColdFrame, FrameCold, FrameEntry},
        frames::ActiveFrameToken,
        stack::FrameStorage,
    };

    fn fixture_source(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
        source: &str,
    ) -> (RunningExecution, FrameId) {
        fixture_source_with_activation(runtime, context, source, true)
    }

    fn fixture_source_with_activation(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
        source: &str,
        materialized: bool,
    ) -> (RunningExecution, FrameId) {
        let Value::Object(function) = context.eval(source).unwrap() else {
            panic!("function");
        };
        let callable = runtime.as_callable(&function).unwrap().unwrap();
        let CallableExecution::Bytecode {
            bytecode,
            closure_slots,
        } = runtime.bytecode_for_callable(&callable).unwrap()
        else {
            panic!("bytecode");
        };
        let prepared = runtime
            .prepare_bytecode_frame(&callable, Value::Undefined, Value::Undefined, &[], bytecode)
            .unwrap();
        let active_frame = if materialized {
            prepared.active_frame.token()
        } else {
            ActiveFrameToken::unmaterialized()
        };
        let entry_guard = materialized.then_some(prepared.active_frame);
        let locals = prepared.locals.len();
        let entry = FrameEntry {
            initialize_bindings: false,
            executable: prepared.executable,
            property_generation: 0,
            iterator_generation: 0,
            caller_realm: context.realm,
            active_frame,
            cold: ColdFrame::new(FrameCold {
                rare: std::cell::OnceCell::new(),
                return_to: None,
                entry_guard,
                function: function.into(),
                closure_slots,
                reusable_captured_locals: vec![false; locals],
                input: prepared.input.into(),
            }),
            storage: FrameStorage {
                original_arguments: vec![],
                parameters: prepared.arguments,
                locals: prepared.locals,
                operands: vec![],
            },
        };
        let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
        let id = crate::engine::vm::driver::push_frame(&mut execution, entry).unwrap();
        (execution, id)
    }

    fn fixture(
        runtime: &Runtime,
        context: &mut crate::engine::api::Context,
    ) -> (RunningExecution, FrameId) {
        fixture_source(runtime, context, "(function(a,b){return a*b})")
    }

    fn push(execution: &mut RunningExecution, id: FrameId, value: JsValue) {
        let frame = execution.frames.current_mut(id).unwrap();
        execution.slots.push(&mut frame.window, value).unwrap();
    }

    fn decoded_fallthrough(execution: &mut RunningExecution, id: FrameId) -> FallthroughPc {
        let frame = execution.frames.current_mut(id).unwrap();
        FallthroughPc::from_decoded(
            frame
                .executable
                .exec
                .decode_published(frame.fault_pc as u32)
                .unwrap(),
        )
    }

    #[test]
    fn numeric_action_carries_the_originating_decoded_boundary() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id) = fixture(&runtime, &mut context);
        let VmAction::Numeric { kind, fallthrough } = execute_frame(&mut execution, id).unwrap()
        else {
            panic!("non-Number multiplication must exit as a numeric action");
        };
        assert_eq!(kind, NumericKind::Mul);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.resume_pc, frame.fault_pc);
        assert_eq!(
            fallthrough.index(),
            frame
                .executable
                .exec
                .decode(frame.fault_pc as u32)
                .unwrap()
                .next_pc as usize
        );
        let fault = frame.fault_pc;
        let (progress, recovery_calls) = crate::engine::vm::frame::count_next_pc_calls(|| {
            try_complete_primitive(&runtime, &mut execution, id, kind, fallthrough)
        });
        assert!(matches!(
            progress.unwrap(),
            Some(NumericProgress::Completed)
        ));
        assert_eq!(recovery_calls, 0);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(
            (frame.fault_pc, frame.resume_pc),
            (fault, fallthrough.index())
        );
    }

    #[test]
    fn wide_compare_fallback_carries_its_own_boundary_without_advancing() {
        use crate::engine::code::exec_opcode::Opcode;

        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id) = fixture_source(
            &runtime,
            &mut context,
            "(function(o,y){if(o.x<y)return 1;return 2})",
        );
        let compare_pc = {
            let frame = execution.frames.current_mut(id).unwrap();
            let exec = &frame.executable.exec;
            (0..exec.instruction_len())
                .filter_map(|source| exec.exec_pc(source as u32))
                .find(|pc| exec.decode(*pc).unwrap().opcode == Opcode::CompareBranchStack)
                .expect("fixture must publish a stack comparison branch")
        };
        push(&mut execution, id, JsValue::Undefined);
        push(&mut execution, id, JsValue::Int(2));
        execution.frames.current_mut(id).unwrap().resume_pc = compare_pc as usize;
        let VmAction::Numeric { kind, fallthrough } = execute_frame(&mut execution, id).unwrap()
        else {
            panic!("non-Number comparison must exit as a numeric action");
        };
        assert_eq!(kind, NumericKind::Lt);
        let frame = execution.frames.current_mut(id).unwrap();
        let reference = frame.executable.exec.decode(compare_pc).unwrap();
        assert!(reference.next_pc > compare_pc + 1);
        assert_eq!(fallthrough.index(), reference.next_pc as usize);
        assert_eq!(frame.fault_pc, compare_pc as usize);
        assert_eq!(frame.resume_pc, compare_pc as usize);
    }

    #[test]
    fn wide_compare_fallback_executes_the_remaining_branch_both_ways() {
        use crate::engine::code::exec_opcode::Opcode;

        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let source = "function compare(o,y){if(o.x<y)return 1;return 2} \
                      compare({x:'1'},'2')===1 && compare({x:'3'},'2')===2";
        let root = context.compile(source).unwrap();
        let child = runtime.test_child_function_bytecode(&root, 0).unwrap();
        assert!(
            runtime
                .test_function_exec_opcodes(&child)
                .unwrap()
                .contains(&Opcode::CompareBranchStack)
        );
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
        #[cfg(feature = "profiling")]
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("numeric_action_exit"),
            Some(&2)
        );
    }

    #[test]
    fn primitive_and_deferred_numeric_throws_keep_the_originating_source_line() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let direct = "function fail(){\n return Symbol()-1;\n}\ntry{fail()}catch(e){e instanceof TypeError && e.stack.includes('at fail (c1-direct.js:2:')}";
        assert_eq!(
            context.eval_with_filename(direct, "c1-direct.js").unwrap(),
            Value::Bool(true)
        );

        let deferred = "let calls=0,marker={};\nfunction fail(){\n return ({valueOf(){calls++;marker.stack=new Error().stack;throw marker}})-1;\n}\ntry{fail()}catch(e){e===marker && calls===1 && marker.stack.includes('at fail (c1-deferred.js:3:')}";
        assert_eq!(
            context
                .eval_with_filename(deferred, "c1-deferred.js")
                .unwrap(),
            Value::Bool(true)
        );
    }

    #[test]
    fn materialization_retry_reexecutes_the_current_operation() {
        use crate::engine::code::exec_opcode::Opcode;

        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id) = fixture_source_with_activation(
            &runtime,
            &mut context,
            "(function(a){return !a})",
            false,
        );
        let not_pc = {
            let frame = execution.frames.current_mut(id).unwrap();
            let exec = &frame.executable.exec;
            (0..exec.instruction_len())
                .filter_map(|source| exec.exec_pc(source as u32))
                .find(|pc| exec.decode(*pc).unwrap().opcode == Opcode::Not)
                .expect("fixture must publish a Not operation")
        };
        let object = runtime.new_object(None).unwrap();
        push(&mut execution, id, JsValue::Object(object.into_handle()));
        execution.frames.current_mut(id).unwrap().resume_pc = not_pc as usize;
        assert_eq!(
            execute_frame(&mut execution, id).unwrap(),
            VmAction::Materialize
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(
            (frame.fault_pc, frame.resume_pc),
            (not_pc as usize, not_pc as usize)
        );
        execution.frames.materialize(&runtime).unwrap();
        assert_eq!(
            execute_frame(&mut execution, id).unwrap(),
            VmAction::Complete
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 0);
        assert_eq!(execution.pending, Some(JsValue::Bool(false)));
    }

    #[test]
    fn direct_post_increment_preserves_pc_after_partial_output_failure() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id) = fixture(&runtime, &mut context);
        loop {
            let frame = execution.frames.current_mut(id).unwrap();
            if execution
                .slots
                .push(&mut frame.window, JsValue::Int(0))
                .is_err()
            {
                break;
            }
        }
        let frame = execution.frames.current_mut(id).unwrap();
        drop(execution.slots.pop(&mut frame.window).unwrap());
        push(&mut execution, id, JsValue::Int(41));
        let fallthrough = decoded_fallthrough(&mut execution, id);
        let before = {
            let frame = execution.frames.current_mut(id).unwrap();
            (frame.fault_pc, frame.resume_pc)
        };
        assert!(
            try_complete_primitive(
                &runtime,
                &mut execution,
                id,
                NumericKind::PostInc,
                fallthrough,
            )
            .is_err()
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), before);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Int(41)
        );
    }

    #[test]
    fn emitted_post_increment_preserves_pc_after_partial_output_failure() {
        use crate::engine::code::exec_opcode::Opcode;

        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id) =
            fixture_source(&runtime, &mut context, "(function(value){return value++})");
        let post_inc_pc = {
            let frame = execution.frames.current_mut(id).unwrap();
            let exec = &frame.executable.exec;
            (0..exec.instruction_len())
                .filter_map(|source| exec.exec_pc(source as u32))
                .find(|pc| exec.decode(*pc).unwrap().opcode == Opcode::PostInc)
                .expect("fixture must publish PostInc")
        };
        loop {
            let frame = execution.frames.current_mut(id).unwrap();
            if execution
                .slots
                .push(&mut frame.window, JsValue::Int(0))
                .is_err()
            {
                break;
            }
        }
        let frame = execution.frames.current_mut(id).unwrap();
        drop(execution.slots.pop(&mut frame.window).unwrap());
        push(&mut execution, id, JsValue::Bool(true));
        execution.frames.current_mut(id).unwrap().resume_pc = post_inc_pc as usize;
        let VmAction::Numeric { kind, fallthrough } = execute_frame(&mut execution, id).unwrap()
        else {
            panic!("non-Number PostInc must produce a numeric action");
        };
        assert_eq!(kind, NumericKind::PostInc);
        let before = {
            let frame = execution.frames.current_mut(id).unwrap();
            (frame.fault_pc, frame.resume_pc)
        };
        assert!(try_complete_primitive(&runtime, &mut execution, id, kind, fallthrough).is_err());
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), before);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Int(1)
        );
    }

    #[test]
    fn string_numeric_completion_uses_carried_fallthrough() {
        use crate::engine::code::exec_opcode::Opcode;

        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let source = "function subtract(a,b){return a-b}; subtract('123.5',2)";
        let root = context.compile(source).unwrap();
        let child = runtime.test_child_function_bytecode(&root, 0).unwrap();
        assert!(
            runtime
                .test_function_exec_opcodes(&child)
                .unwrap()
                .contains(&Opcode::Sub)
        );
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(context.eval(source).unwrap(), Value::Float(121.5));
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("numeric_action_exit"), Some(&1));
            assert_eq!(
                events.get("numeric_completed_with_carried_fallthrough"),
                Some(&1)
            );
            assert_eq!(
                events
                    .get("numeric_legacy_fallthrough_recovery_decode")
                    .copied()
                    .unwrap_or(0),
                0
            );
        }
    }

    #[test]
    fn primitive_numeric_domains_complete_at_the_carried_boundary() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    "function subtract(a,b){return a-b}; \
                     subtract(true,1)===0 && subtract(null,2)===-2 && subtract(7n,2n)===5n"
                )
                .unwrap(),
            Value::Bool(true)
        );
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(
                events.get("numeric_completed_with_carried_fallthrough"),
                Some(&3)
            );
            assert_eq!(
                events
                    .get("numeric_legacy_fallthrough_recovery_decode")
                    .copied()
                    .unwrap_or(0),
                0
            );
        }
    }

    #[test]
    fn number_only_arithmetic_needs_no_numeric_action() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval("function subtract(a,b){return a-b}; subtract(123.5,2)")
                .unwrap(),
            Value::Float(121.5)
        );
        #[cfg(feature = "profiling")]
        assert_eq!(
            profile
                .snapshot()
                .owned_execution_events
                .get("numeric_action_exit")
                .copied()
                .unwrap_or(0),
            0
        );
    }

    #[test]
    fn object_numeric_decline_keeps_throw_identity_and_conversion_count() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    "(() => { let calls=0, marker={}; \
                     try { ({valueOf(){calls++;throw marker}})-1 } \
                     catch(error) { return error===marker && calls===1 } \
                     return false })()"
                )
                .unwrap(),
            Value::Bool(true)
        );
        #[cfg(feature = "profiling")]
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("numeric_primitive_declined")
                .copied()
                .unwrap_or(0)
                > 0
        );
    }

    #[test]
    fn primitive_transaction_preserves_partial_numeric_input_and_output_errors() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id) = fixture(&runtime, &mut context);
        push(&mut execution, id, JsValue::Int(7));
        let fallthrough = decoded_fallthrough(&mut execution, id);
        let original_positions = {
            let frame = execution.frames.current_mut(id).unwrap();
            (frame.fault_pc, frame.resume_pc)
        };
        assert!(
            try_complete_primitive(&runtime, &mut execution, id, NumericKind::Mul, fallthrough)
                .unwrap()
                .is_none()
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), original_positions);
        assert_eq!(execution.slots.depth(&frame.window), 1);
        assert!(complete(&runtime, &mut execution, id, NumericKind::Mul).is_err());
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(
            execution.slots.depth(&frame.window),
            0,
            "outer RHS consumption must survive missing LHS"
        );
        // Leave exactly one free slot: previous commits even when value cannot.
        loop {
            let frame = execution.frames.current_mut(id).unwrap();
            if execution
                .slots
                .push(&mut frame.window, JsValue::Int(0))
                .is_err()
            {
                break;
            }
        }
        let frame = execution.frames.current_mut(id).unwrap();
        drop(execution.slots.pop(&mut frame.window).unwrap());
        let fault = frame.fault_pc;
        let resume = frame.resume_pc;
        assert!(
            commit_output(
                &runtime,
                &mut execution,
                id,
                JsValue::Int(99),
                Some(JsValue::Int(41)),
                0
            )
            .is_err()
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Int(41)
        );
        assert_eq!((frame.fault_pc, frame.resume_pc), (fault, resume));
    }

    #[test]
    fn primitive_transaction_identity_domain_and_wait_order() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (mut execution, id) = fixture(&runtime, &mut context);
        execution
            .frames
            .current_mut(id)
            .unwrap()
            .property_generation = u64::MAX;
        push(&mut execution, id, JsValue::Int(6));
        push(&mut execution, id, JsValue::Int(7));
        let fallthrough = decoded_fallthrough(&mut execution, id);
        assert!(matches!(
            try_complete_primitive(&runtime, &mut execution, id, NumericKind::Mul, fallthrough)
                .unwrap(),
            Some(NumericProgress::Completed)
        ));
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.resume_pc, fallthrough.index());
        assert_eq!(frame.property_generation, u64::MAX);
        assert_eq!(
            execution.slots.pop(&mut frame.window).unwrap(),
            JsValue::Int(42)
        );
        assert!(matches!(
            crate::engine::vm::proxy_get_driver::start_numeric(
                &runtime,
                &mut execution,
                id,
                NumericStep::Throw(JsValue::Int(17)),
                0
            )
            .unwrap(),
            NumericProgress::Deferred(CallStep::Complete(crate::engine::vm::Completion::Throw(
                JsValue::Int(17)
            )))
        ));
        let object = runtime.new_object(None).unwrap();
        let step = NumericStep::start(
            &runtime,
            NumericKind::Plus,
            JsValue::Object(object.object_id()),
            None,
        )
        .unwrap();
        assert!(
            crate::engine::vm::proxy_get_driver::start_numeric(
                &runtime,
                &mut execution,
                id,
                step,
                0
            )
            .is_err()
        );
        push(&mut execution, id, JsValue::Object(object.into_handle()));
        let mut identity = u64::MAX;
        assert!(complete_primitives(&runtime, &mut execution, id, false, &mut identity).is_err());
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 1);
        identity = 10;
        assert!(matches!(
            complete_primitives(&runtime, &mut execution, id, false, &mut identity).unwrap(),
            PrimitiveCompletion::Declined
        ));
        assert_eq!(identity, 11);
        let frame = execution.frames.current_mut(id).unwrap();
        runtime
            .release_jsvalue(execution.slots.pop(&mut frame.window).unwrap())
            .unwrap();
        let foreign = Runtime::new();
        push(
            &mut execution,
            id,
            JsValue::Object(foreign.new_object(None).unwrap().into_handle()),
        );
        identity = 10;
        assert!(matches!(
            complete_primitives(&runtime, &mut execution, id, false, &mut identity).unwrap(),
            PrimitiveCompletion::Declined
        ));
        assert_eq!(identity, 11);
        let frame = execution.frames.current_mut(id).unwrap();
        let pending = execution.slots.pop(&mut frame.window).unwrap();
        foreign.release_jsvalue(pending).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 0);
    }

    #[test]
    fn primitive_transaction_parsing_and_string_store_keep_shared_semantics() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(r#"(()=>{
            let cases=['','  \t\n',' 1.25e2 ','0x10','0b101','0o17','Infinity','-Infinity','-0','bad','\ud800'];
            for(let s of cases) {
                let direct=s-0, callback=({valueOf(){return s}})-0;
                if(!Object.is(direct,callback)||!Object.is(+s,+({valueOf(){return s}})))return false;
            }
            let s='', alias='', b=1n;
            for(let i=0;i<40;i++){alias=s;s+='\ud800x';s='y'+s;b=b*3n;b+=1n;b+=2n;}
            if(s.length!==120||alias.length!==117||typeof b!=='bigint')return false;
            let marker={}, trace='', local='old';
            try{local+= { [Symbol.toPrimitive](hint){trace+=hint;throw marker} }}
            catch(e){if(e!==marker)return false;trace+='caught'}finally{trace+='finally'}
            let capture=()=>local;
            local+='new';
            return capture()==='oldnew' && trace==='defaultcaughtfinally';
        })()"#).unwrap(), Value::Bool(true));
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn copy_transaction_exhausted_wait_restores_source_without_replaying_selected_getter() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let source = context
            .eval("globalThis.copyCalls=0;({a:1,get b(){copyCalls++;return 2}})")
            .unwrap();
        let target = runtime.new_object(None).unwrap();
        let (mut execution, id) = fixture(&runtime, &mut context);
        execution
            .frames
            .current_mut(id)
            .unwrap()
            .property_generation = u64::MAX;
        push(
            &mut execution,
            id,
            JsValue::Object(target.clone().into_handle()),
        );
        push(
            &mut execution,
            id,
            runtime.into_jsvalue(source.clone()).unwrap(),
        );
        let result = crate::engine::vm::proxy_get_driver::start_object_copy(
            &runtime,
            &mut execution,
            id,
            1,
            0,
            None,
        );
        assert!(
            matches!(result, Err(ref error) if error.message()=="copy query identity exhausted")
        );
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            runtime
                .root_value(execution.slots.peek(&frame.window, 0).unwrap())
                .unwrap(),
            source
        );
        assert_eq!(frame.property_generation, u64::MAX);
        // Classification may already have completed ordinary fresh-target
        // definitions. No selected getter was called or replayed to discover it.
        assert!(
            runtime
                .get_own_property(&target, &runtime.intern_property_key("a").unwrap())
                .unwrap()
                .is_some()
        );
        assert!(
            runtime
                .get_own_property(&target, &runtime.intern_property_key("b").unwrap())
                .unwrap()
                .is_none()
        );
        drop(execution);
        assert_eq!(context.eval("copyCalls").unwrap(), Value::Int(0));
    }

    #[test]
    fn copy_transaction_sync_exhaustion_and_failure_keep_input_commit_boundary() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        for fail in [false, true] {
            let source = context.eval("({a:1})").unwrap();
            let target = context
                .eval(if fail { "Object.freeze({})" } else { "({})" })
                .unwrap();
            let (mut execution, id) = fixture(&runtime, &mut context);
            execution
                .frames
                .current_mut(id)
                .unwrap()
                .property_generation = u64::MAX;
            push(&mut execution, id, runtime.into_jsvalue(target).unwrap());
            push(&mut execution, id, runtime.into_jsvalue(source).unwrap());
            let result = crate::engine::vm::proxy_get_driver::start_object_copy(
                &runtime,
                &mut execution,
                id,
                1,
                0,
                None,
            );
            if fail {
                assert!(
                    result.is_err()
                        || matches!(
                            result,
                            Ok(CallStep::Complete(crate::engine::vm::Completion::Throw(_)))
                        )
                );
            } else {
                assert!(matches!(result, Ok(CallStep::Entered)));
            }
            let frame = execution.frames.current_mut(id).unwrap();
            assert_eq!(
                execution.slots.depth(&frame.window),
                1,
                "source consumed before the first ordinary copy effect"
            );
            assert_eq!(frame.property_generation, u64::MAX);
        }
    }

    #[test]
    fn primitive_numeric_completion_keeps_bigint_operators_and_two_update_results() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let x=5n, old=x++, next=++x, removed=x--, last=--x;
            let a=6n, b=3n;
            let values=[a*b,a/b,a%b,a**b,a<<b,a>>b,a&b,a|b,a^b,~a,-a];
            let errors='';
            try{a/0n}catch(e){errors+=e instanceof RangeError?'z':'!'}
            try{a*2}catch(e){errors+=e instanceof TypeError?'m':'!'}
            try{a>>>b}catch(e){errors+=e instanceof TypeError?'u':'!'}
            try{Symbol()*2}catch(e){errors+=e instanceof TypeError?'s':'!'}
            finally{errors+='f'}
            return values.join(',')==='18,2,0,216,48,0,2,7,5,-7,-6'
                && old===5n && next===7n && removed===7n && last===5n && x===5n
                && errors==='zmusf';
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        #[cfg(feature = "profiling")]
        assert!(
            profile
                .snapshot()
                .owned_execution_events
                .get("numeric_completed_in_same_frame")
                .copied()
                .unwrap_or(0)
                >= 10
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }

    #[test]
    fn object_numeric_fallback_keeps_conversion_order_throw_identity_and_finally() {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(
            context
                .eval(
                    r#"(()=>{
            let trace='', marker={}, left={valueOf(){trace+='l';return 6n}},
                right={valueOf(){trace+='r';return 3n}};
            if(left*right!==18n)throw 'product';
            try{({valueOf(){trace+='x';throw marker}})*right}catch(e){if(e===marker)trace+='t'}
            finally{trace+='f'}
            let target={valueOf(){trace+='p';return 8n}};
            let old=target++;
            const fixed={valueOf(){trace+='c';return 2n}};
            try{fixed++}catch(e){if(e instanceof TypeError)trace+='e'}
            return trace==='lrxtfpce' && old===8n && target===9n;
        })()"#
                )
                .unwrap(),
            Value::Bool(true)
        );
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}
