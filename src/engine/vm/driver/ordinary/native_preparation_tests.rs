//! A recoverable observation failure must not orphan transferred native inputs.
use super::enter_selected;
use crate::engine::{
    api::{Runtime, Value},
    builtins::native::NativeFunctionId,
    code::{
        bytecode::Instruction,
        function::{UnlinkedFunction, metadata::FunctionMetadata},
    },
    heap::ContextId,
    value::JsValue,
    vm::{
        call::CallableExecution,
        execute::FallthroughPc,
        execution::{ExecutionLimits, RunningExecution},
        frame::{FrameEntry, FrameId},
        frames::ActiveFrameToken,
    },
};

fn lazy_caller(runtime: &Runtime, realm: ContextId, method: bool) -> FrameEntry {
    let depth = if method { 5 } else { 4 };
    let mut instructions = vec![Instruction::Undefined; depth];
    instructions.push(if method {
        Instruction::CallMethod(2)
    } else {
        Instruction::Call(2)
    });
    instructions.push(Instruction::Return);
    let bytecode = runtime
        .publish_unlinked_function(
            realm,
            UnlinkedFunction::fixture(
                instructions,
                Vec::new(),
                FunctionMetadata {
                    max_stack: depth as u16,
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
        panic!("bytecode caller");
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

fn install(
    runtime: &Runtime,
    execution: &mut RunningExecution,
    realm: ContextId,
    method: bool,
) -> (FrameId, FallthroughPc) {
    let entry = lazy_caller(runtime, realm, method);
    let id = crate::engine::vm::driver::push_frame(runtime, execution, entry).unwrap();
    let frame = execution.frames.current_mut(id).unwrap();
    let pc = if method { 5 } else { 4 };
    frame.fault_pc = pc;
    frame.resume_pc = pc;
    let decoded = frame.executable.exec.decode_published(pc as u32).unwrap();
    (id, FallthroughPc::from_decoded(decoded))
}

fn push(runtime: &Runtime, execution: &mut RunningExecution, id: FrameId, value: JsValue) {
    let frame = execution.frames.current_mut(id).unwrap();
    execution.slots.push(&mut frame.window, value).unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn native_materialization_failure_releases_inputs_and_preserves_lower_slots() {
    for (resident, method, aliased) in [
        (false, false, false),
        (false, true, false),
        (false, true, true),
        (true, false, false),
        (true, true, false),
        (true, true, true),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().unwrap();
        let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
        let (id, fallthrough) = install(&runtime, &mut execution, context.realm, method);
        let prototype = context.function_prototype().unwrap();
        let callable = runtime
            .new_bound_native_function(
                &prototype,
                context.realm,
                if resident {
                    NativeFunctionId::MathRandom
                } else {
                    NativeFunctionId::ArgumentProbe
                },
                if resident { 0 } else { 2 },
            )
            .unwrap()
            .into_object()
            .into_handle();
        drop(prototype);
        let marker = runtime.new_object(None).unwrap().into_handle();
        let surviving = aliased.then(|| runtime.new_object(None).unwrap());
        let mut inputs = Vec::new();
        for _ in 0..2 + usize::from(method) {
            let value = if let Some(root) = &surviving {
                runtime
                    .dup_jsvalue(&JsValue::Object(root.object_id()))
                    .unwrap()
            } else {
                JsValue::Object(runtime.new_object(None).unwrap().into_handle())
            };
            inputs.push(value);
        }
        let input_ids = inputs
            .iter()
            .map(|value| match value {
                JsValue::Object(id) => *id,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();
        push(&runtime, &mut execution, id, JsValue::Object(marker));
        if method {
            push(&runtime, &mut execution, id, inputs.remove(0));
        }
        push(&runtime, &mut execution, id, JsValue::Object(callable));
        for value in inputs {
            push(&runtime, &mut execution, id, value);
        }

        // Keep the old callee-transfer drain before the failing observation.
        let pending = runtime.new_object(None).unwrap();
        let pending_id = pending.object_id();
        {
            let state = runtime.0.state.borrow();
            drop(pending);
            assert!(state.heap.object(pending_id).is_ok());
        }
        assert!(runtime.0.deferred_references.has_pending());
        let saved_token = runtime.0.state.borrow().next_active_frame_token;
        runtime.0.state.borrow_mut().next_active_frame_token = u64::MAX;
        let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
        let entry = if resident {
            let mut state = runtime.0.state.borrow_mut();
            crate::engine::vm::stack::FrameExecution::admit(&mut execution, id)
                .unwrap()
                .enter_ordinary(&runtime, &mut state, 2, method, false, fallthrough)
        } else {
            enter_selected(
                &runtime,
                &mut execution,
                id,
                2,
                method,
                false,
                None,
                fallthrough,
            )
        };
        let error = entry.err().expect("frame token exhaustion");
        assert_eq!(
            error.message(),
            "runtime invariant failed: active-frame token space was exhausted"
        );
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
        let frame = execution.frames.current_mut(id).unwrap();
        let pc = if method { 5 } else { 4 };
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert!(!frame.active_frame.is_materialized());
        assert_eq!(execution.slots.depth(&frame.window), 1);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Object(marker)
        );
        {
            let state = runtime.0.state.borrow();
            assert!(state.heap.object(callable).is_err());
            assert!(state.heap.object(pending_id).is_err());
            assert_eq!(state.heap.object_strong_count(marker), Ok(1));
            for input in input_ids {
                if aliased {
                    assert_eq!(state.heap.object_strong_count(input), Ok(1));
                } else {
                    assert!(state.heap.object(input).is_err());
                }
            }
            assert!(state.active_frames.is_empty());
        }
        assert!(!runtime.0.deferred_references.has_pending());
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
        assert!(!runtime.is_poisoned());
        runtime.0.state.borrow_mut().next_active_frame_token = saved_token;
        drop(execution);
        assert!(runtime.0.state.borrow().heap.object(marker).is_err());
        drop(surviving);
        assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
    }
}
