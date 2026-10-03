//! Actual resident closure creation owns its captures before allocation/GC.
use crate::engine::{
    api::{Runtime, Value},
    code::{
        bytecode::Instruction,
        function::{
            UnlinkedFunction, UnlinkedVariableDefinition,
            metadata::{
                ClosureSource, ClosureVariable, ClosureVariableKind, ClosureVariableName,
                FunctionKind, FunctionMetadata,
            },
        },
    },
    heap::{ContextId, ObjectId, ObjectPayload, RawId},
    value::JsValue,
    vm::{
        bindings::FrameBinding,
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
        frame::{FrameEntry, FrameId},
        frames::ActiveFrameToken,
    },
};

fn entry(runtime: &Runtime, realm: ContextId, kind: FunctionKind) -> FrameEntry {
    let code = if matches!(kind, FunctionKind::Generator | FunctionKind::AsyncGenerator) {
        vec![
            Instruction::InitialYield,
            Instruction::GetVarRef(0),
            Instruction::Return,
        ]
    } else {
        vec![Instruction::GetVarRef(0), Instruction::Return]
    };
    let child = UnlinkedFunction::fixture_with_closure_variables(
        code,
        Vec::new(),
        FunctionMetadata {
            closure_count: 1,
            max_stack: 1,
            function_kind: kind,
            has_prototype: matches!(kind, FunctionKind::Generator | FunctionKind::AsyncGenerator),
            ..Default::default()
        },
        vec![ClosureVariable {
            source: ClosureSource::ParentLocal(0),
            name: ClosureVariableName::None,
            is_lexical: false,
            is_const: false,
            kind: ClosureVariableKind::Normal,
        }],
    );
    let parent = runtime
        .publish_unlinked_function(
            realm,
            UnlinkedFunction::fixture(
                vec![Instruction::FClosure(0), Instruction::Return],
                vec![crate::engine::code::function::UnlinkedConstant::child(
                    child,
                )],
                FunctionMetadata {
                    local_count: 1,
                    max_stack: 1,
                    ..Default::default()
                },
            )
            .with_fixture_definitions(Vec::new(), vec![UnlinkedVariableDefinition::ordinary(None)]),
        )
        .unwrap();
    let callable = runtime.new_bytecode_closure(realm, &parent).unwrap();
    let CallableExecution::Bytecode {
        bytecode,
        closure_slots,
    } = runtime.bytecode_for_callable(&callable).unwrap()
    else {
        panic!("bytecode callable")
    };
    let mut entry = crate::engine::vm::root_call::prepare_call(
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
    entry
        .cold
        .entry_guard
        .take()
        .unwrap()
        .finish(&mut runtime.0.state.borrow_mut())
        .unwrap();
    entry.active_frame = ActiveFrameToken::unmaterialized();
    entry
}

fn execution(
    runtime: &Runtime,
    realm: ContextId,
    kind: FunctionKind,
    object: ObjectId,
) -> (RunningExecution, FrameId) {
    let entry = entry(runtime, realm, kind);
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    let frame = execution.frames.current_mut(id).unwrap();
    let previous = execution
        .slots
        .replace_local(
            &frame.window,
            0,
            FrameBinding::Direct(JsValue::Object(object)),
        )
        .unwrap();
    assert!(matches!(previous, FrameBinding::Direct(JsValue::Undefined)));
    (execution, id)
}

fn clean(runtime: &Runtime) {
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn resident_closure_keeps_captured_object_and_result_live_before_collection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(garbage) = context
        .eval("(()=>{let o={};o.self=o;return o})()")
        .unwrap()
    else {
        panic!("cyclic object")
    };
    let garbage = garbage.into_handle();
    runtime.release_jsvalue(JsValue::Object(garbage)).unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id) = execution(&runtime, context.realm, FunctionKind::Normal, marker);
    let owners = std::rc::Rc::strong_count(&runtime.0);
    runtime.0.gc_pressure.remaining.set(1);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let (function, cell);
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        let Some(JsValue::Object(result)) = execution.pending.as_ref() else {
            panic!("owned closure result")
        };
        function = *result;
        let ObjectPayload::BytecodeFunction { closure_slots, .. } =
            &state.heap.object(function).unwrap().payload
        else {
            panic!("bytecode closure")
        };
        cell = closure_slots[0];
        assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert_eq!(state.heap.object_strong_count(function), Ok(1));
        assert!(state.heap.object(garbage).is_err());
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(!runtime.0.deferred_references.has_pending());
    }
    #[cfg(feature = "profiling")]
    {
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
        assert_eq!(events.get("core.internal_closure"), Some(&1));
        assert_eq!(events.get("runtime.clone"), None);
        assert_eq!(events.get("runtime.deferred.release"), None);
    }
    drop(execution);
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(function).is_err());
    assert!(state.heap.object(marker).is_err());
    assert!(state.heap.var_ref(cell).is_err());
    drop(state);
    clean(&runtime);
}

#[test]
fn resident_closure_rejected_output_retires_result_and_capture_temporary() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id) = execution(&runtime, context.realm, FunctionKind::Normal, marker);
    let input = runtime.new_object(None).unwrap().into_handle();
    let frame = execution.frames.current_mut(id).unwrap();
    execution
        .slots
        .push(&mut frame.window, JsValue::Object(input))
        .unwrap();
    {
        let mut state = runtime.0.state.borrow_mut();
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert!(
            error
                .message()
                .contains("owned operand stack exceeds verified capacity")
        );
        let frame = execution.frames.current_mut(id).unwrap();
        let FrameBinding::Captured(cell) = execution.slots.local(&frame.window, 0).unwrap() else {
            panic!("capture prefix was published")
        };
        assert_eq!(state.heap.var_ref_strong_count(*cell), Ok(1));
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Object(input)
        );
        assert_eq!((frame.fault_pc, frame.resume_pc), (0, 0));
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    assert!(runtime.0.state.borrow().heap.object(input).is_err());
    clean(&runtime);
}

#[test]
fn resident_closure_checked_bytecode_retain_failure_preserves_direct_binding() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id) = execution(&runtime, context.realm, FunctionKind::Normal, marker);
    {
        let mut state = runtime.0.state.borrow_mut();
        let frame = execution.frames.current_mut(id).unwrap();
        let Some(crate::engine::heap::BytecodeConstant::Function(child)) =
            frame.executable.constant(0)
        else {
            panic!("child constant")
        };
        let child = *child;
        let count = state.heap.function_bytecode_strong_count(child).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::FunctionBytecode(child), u32::MAX);
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        state
            .heap
            .set_strong_count_for_test(RawId::FunctionBytecode(child), count);
        assert!(error.message().contains("overflow"));
        let frame = execution.frames.current_mut(id).unwrap();
        assert!(
            matches!(execution.slots.local(&frame.window, 0).unwrap(), FrameBinding::Direct(JsValue::Object(value)) if *value == marker)
        );
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert_eq!((frame.fault_pc, frame.resume_pc), (0, 0));
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_generator_prototype_checked_failure_keeps_published_capture_and_can_retry() {
    for kind in [FunctionKind::Generator, FunctionKind::AsyncGenerator] {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let marker = runtime.new_object(None).unwrap().into_handle();
        let (mut execution, id) = execution(&runtime, context.realm, kind, marker);
        {
            let mut state = runtime.0.state.borrow_mut();
            let context_data = state.heap.context(context.realm).unwrap();
            let prototype = if kind == FunctionKind::Generator {
                context_data.generator.unwrap().prototype
            } else {
                context_data.async_generator.unwrap().prototype
            };
            let count = state.heap.object_strong_count(prototype).unwrap();
            state
                .heap
                .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
            let error = super::execute_frame_in_state(&runtime, &mut state, &mut execution, id)
                .unwrap_err();
            state
                .heap
                .set_strong_count_for_test(RawId::Object(prototype), count);
            assert!(error.message().contains("overflow"));
            let frame = execution.frames.current_mut(id).unwrap();
            let FrameBinding::Captured(cell) = execution.slots.local(&frame.window, 0).unwrap()
            else {
                panic!("capture prefix")
            };
            assert_eq!(state.heap.var_ref_strong_count(*cell), Ok(1));
            assert_eq!(state.heap.object_strong_count(marker), Ok(1));
            assert_eq!((frame.fault_pc, frame.resume_pc), (0, 0));
            assert!(!runtime.is_poisoned());
            assert!(matches!(
                super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
                super::VmAction::Complete
            ));
        }
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(marker).is_err());
        clean(&runtime);
    }
}

#[test]
fn resident_closure_publication_failure_quarantines_before_capture_temporaries() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id) = execution(&runtime, context.realm, FunctionKind::Normal, marker);
    let prototype = runtime
        .0
        .state
        .borrow()
        .heap
        .context(context.realm)
        .unwrap()
        .function_prototype;
    let prototype_root =
        crate::engine::object::ObjectRef::from_borrowed_handle(runtime.clone(), prototype).unwrap();
    let _matching_layout = runtime.new_object(Some(&prototype_root)).unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    {
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .queue_release_for_test(RawId::Object(first))
            .unwrap();
        state
            .heap
            .queue_release_for_test(RawId::Object(later))
            .unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(first), 1);
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert!(
            error
                .message()
                .contains("finalization count disagrees with its queue/cycle state")
        );
        assert!(runtime.is_poisoned());
        let frame = execution.frames.current_mut(id).unwrap();
        let FrameBinding::Captured(cell) = execution.slots.local(&frame.window, 0).unwrap() else {
            panic!("capture prefix")
        };
        // Frame, capture temporary and interrupted published function each
        // still own a cell edge. Poison prevents traversing either temporary.
        assert_eq!(state.heap.var_ref_strong_count(*cell), Ok(3));
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert_eq!(state.heap.object_strong_count(later), Ok(0));
        assert!(state.heap.has_pending_zero_cleanup());
        assert_eq!((frame.fault_pc, frame.resume_pc), (0, 0));
    }
    drop(execution);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(marker),
        Ok(1)
    );
}

#[test]
fn resident_closure_materialization_failure_precedes_capture_and_allocation() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let marker = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id) = execution(&runtime, context.realm, FunctionKind::Normal, marker);
    {
        let mut state = runtime.0.state.borrow_mut();
        state.next_active_frame_token = u64::MAX;
        let before = state.heap.counts();
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert!(
            error
                .message()
                .contains("active-frame token space was exhausted")
        );
        assert_eq!(state.heap.counts(), before);
        let frame = execution.frames.current_mut(id).unwrap();
        assert!(
            matches!(execution.slots.local(&frame.window, 0).unwrap(), FrameBinding::Direct(JsValue::Object(value)) if *value == marker)
        );
        assert_eq!((frame.fault_pc, frame.resume_pc), (0, 0));
        assert!(!frame.active_frame.is_materialized());
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    clean(&runtime);
}
