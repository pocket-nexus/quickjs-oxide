//! Resident OP_object publishes ownership and observation before GC service.
use crate::engine::{
    api::{Runtime, Value},
    code::{
        bytecode::Instruction,
        function::{UnlinkedFunction, metadata::FunctionMetadata},
    },
    heap::ContextId,
    value::JsValue,
    vm::{
        BytecodePc,
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
        frame::{FrameEntry, FrameId},
        frames::{ActiveFrameKind, ActiveFrameToken},
    },
};

fn object_entry(runtime: &Runtime, realm: ContextId, caller_realm: ContextId) -> FrameEntry {
    let bytecode = runtime
        .publish_unlinked_function(
            realm,
            UnlinkedFunction::fixture(
                vec![Instruction::Object, Instruction::Return],
                Vec::new(),
                FunctionMetadata {
                    max_stack: 1,
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
        panic!("published bytecode callable");
    };
    let mut entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        caller_realm,
        &callable,
        JsValue::Undefined,
        JsValue::Undefined,
        Vec::new(),
        bytecode,
        closure_slots,
    )
    .unwrap();
    // Exercise the lazy registration used by ordinary children, while keeping
    // their existing function/executable owners in the prepared frame.
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

fn object_execution(runtime: &Runtime, realm: ContextId) -> (RunningExecution, FrameId) {
    let entry = object_entry(runtime, realm, realm);
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    (execution, id)
}

fn clean(runtime: &Runtime) {
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn resident_object_publishes_one_owner_and_fault_metadata_before_pressure_collection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(garbage) = context
        .eval("(()=>{let garbage={};garbage.self=garbage;return garbage})()")
        .unwrap()
    else {
        panic!("cyclic garbage");
    };
    let garbage = garbage.into_handle();
    runtime.release_jsvalue(JsValue::Object(garbage)).unwrap();
    let (mut execution, id) = object_execution(&runtime, context.realm);
    let owners = std::rc::Rc::strong_count(&runtime.0);
    runtime.0.gc_pressure.remaining.set(1);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let object;
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(state.heap.object(garbage).is_ok());
        assert!(matches!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        let Some(JsValue::Object(result)) = execution.pending.as_ref() else {
            panic!("OP_object must complete in the resident executor");
        };
        object = *result;
        assert_eq!(state.heap.object_strong_count(object), Ok(1));
        let shape = state.heap.object(object).unwrap().shape;
        assert_eq!(
            state.heap.shape(shape).unwrap().prototype(),
            Some(state.heap.context(context.realm).unwrap().object_prototype)
        );
        assert!(state.heap.object(garbage).is_err());
        assert!(runtime.0.gc_pressure.remaining.get() > 0);
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(!runtime.0.deferred_references.has_pending());
        let frame = execution.frames.current_mut(id).unwrap();
        assert!(frame.active_frame.is_materialized());
        assert_eq!(state.active_frames.len(), 1);
        assert_eq!(runtime.0.active_frame_depth.get(), 1);
        let record = state.active_frames.last().unwrap();
        assert_eq!(record.token, frame.active_frame);
        assert_eq!(record.function, frame.cold.function.object_id());
        assert_eq!(record.realm, context.realm);
        assert_eq!(
            record.kind,
            ActiveFrameKind::Bytecode {
                bytecode: frame.executable.bytecode_id().unwrap(),
                pc: Some(BytecodePc::new(0)),
            }
        );
    }
    #[cfg(feature = "profiling")]
    {
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events.get("core.internal_object"), Some(&1));
        assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
        assert_eq!(events.get("runtime.clone"), None);
        assert_eq!(events.get("runtime.deferred.release"), None);
    }
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(object).is_err());
    clean(&runtime);
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
}

#[test]
fn resident_object_uses_the_executable_realm_when_the_caller_realm_differs() {
    let runtime = Runtime::new();
    let first = runtime.new_context().unwrap();
    let second = runtime.new_context().unwrap();
    let entry = object_entry(&runtime, second.realm, first.realm);
    let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(&runtime, &mut execution, entry).unwrap();
    // Publish the frame so its realm record is observable below.
    runtime.0.gc_pressure.remaining.set(1);
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        let Some(JsValue::Object(object)) = execution.pending.as_ref() else {
            panic!("resident object result");
        };
        let shape = state.heap.object(*object).unwrap().shape;
        let prototype = state.heap.shape(shape).unwrap().prototype();
        assert_eq!(
            prototype,
            Some(state.heap.context(second.realm).unwrap().object_prototype)
        );
        assert_ne!(
            prototype,
            Some(state.heap.context(first.realm).unwrap().object_prototype)
        );
        assert_eq!(state.active_frames.last().unwrap().realm, second.realm);
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_object_rejected_commit_releases_fresh_edge_and_keeps_fault_and_input() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let (mut execution, id) = object_execution(&runtime, context.realm);
    let marker = runtime.new_object(None).unwrap().into_handle();
    let frame = execution.frames.current_mut(id).unwrap();
    execution
        .slots
        .push(&mut frame.window, JsValue::Object(marker))
        .unwrap();
    runtime.0.gc_pressure.remaining.set(1);
    {
        let mut state = runtime.0.state.borrow_mut();
        let before = state.heap.counts();
        let prototype = state.heap.context(context.realm).unwrap().object_prototype;
        let prototype_count = state.heap.object_strong_count(prototype).unwrap();
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert!(
            error
                .message()
                .contains("owned operand stack exceeds verified capacity")
        );
        // Releasing the fresh edge can grow reusable vacant capacity. Its
        // live object/layout owners and cleanup queues must return to baseline.
        let after = state.heap.counts();
        assert_eq!(after.object_nodes, before.object_nodes);
        assert_eq!(after.shape_nodes, before.shape_nodes);
        assert_eq!(after.live, before.live);
        assert_eq!(after.initializing, before.initializing);
        assert_eq!(after.zero_queued, before.zero_queued);
        assert_eq!(
            state.heap.object_strong_count(prototype),
            Ok(prototype_count)
        );
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (0, 0));
        assert_eq!(execution.slots.depth(&frame.window), 1);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Object(marker)
        );
        assert_eq!(
            state.active_frames.last().unwrap().kind,
            ActiveFrameKind::Bytecode {
                bytecode: frame.executable.bytecode_id().unwrap(),
                pc: Some(BytecodePc::new(0)),
            }
        );
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    clean(&runtime);
}

#[test]
fn resident_object_materialization_failure_does_not_allocate_or_advance() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let (mut execution, id) = object_execution(&runtime, context.realm);
    // Near a collection the frames are published before allocating.
    runtime.0.gc_pressure.remaining.set(1);
    {
        let mut state = runtime.0.state.borrow_mut();
        state.next_active_frame_token = u64::MAX;
        let before = state.heap.counts();
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert_eq!(
            error.message(),
            "runtime invariant failed: active-frame token space was exhausted"
        );
        assert_eq!(state.heap.counts(), before);
        assert!(state.active_frames.is_empty());
        let frame = execution.frames.current_mut(id).unwrap();
        assert!(!frame.active_frame.is_materialized());
        assert_eq!((frame.fault_pc, frame.resume_pc), (0, 0));
        assert_eq!(execution.slots.depth(&frame.window), 0);
        assert_eq!(execution.frames.logical_active_depth(&runtime), 1);
        assert_eq!(runtime.0.active_frame_depth.get(), 0);
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn direct_materialization_preserves_partial_suffix_and_retries_without_duplicate_ancestors() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let parent = object_entry(&runtime, context.realm, context.realm);
    let child = object_entry(&runtime, context.realm, context.realm);
    let mut execution = RunningExecution::new(&runtime, ExecutionLimits::default()).unwrap();
    crate::engine::vm::driver::push_frame(&runtime, &mut execution, parent).unwrap();
    let child = crate::engine::vm::driver::push_frame(&runtime, &mut execution, child).unwrap();
    {
        let mut state = runtime.0.state.borrow_mut();
        state.next_active_frame_token = u64::MAX - 1;
        let before = state.heap.counts();
        assert_eq!(
            execution
                .frames
                .materialize_in_state(&mut state)
                .unwrap_err()
                .message(),
            "runtime invariant failed: active-frame token space was exhausted"
        );
        assert_eq!(state.heap.counts(), before);
        assert_eq!(state.active_frames.len(), 1);
        assert_eq!(runtime.0.active_frame_depth.get(), 1);
        assert_eq!(execution.frames.logical_active_depth(&runtime), 2);
        let registered_parent = state.active_frames.last().unwrap().token;
        assert!(
            !execution
                .frames
                .current_mut(child)
                .unwrap()
                .active_frame
                .is_materialized()
        );
        state.next_active_frame_token = 100;
        execution.frames.materialize_in_state(&mut state).unwrap();
        assert_eq!(state.active_frames.len(), 2);
        assert_eq!(state.active_frames.get(0).unwrap().token, registered_parent);
        assert_eq!(runtime.0.active_frame_depth.get(), 2);
        assert_eq!(execution.frames.logical_active_depth(&runtime), 2);
        let next_token = state.next_active_frame_token;
        execution.frames.current_mut(child).unwrap().fault_pc = 1;
        execution.frames.materialize_in_state(&mut state).unwrap();
        assert_eq!(state.next_active_frame_token, next_token);
        assert_eq!(state.active_frames.len(), 2);
        assert_eq!(state.active_frames.get(0).unwrap().token, registered_parent);
        assert!(matches!(state.active_frames.last().unwrap().kind,
            ActiveFrameKind::Bytecode { pc: Some(pc), .. } if pc == BytecodePc::new(1)));
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_object_with_headroom_completes_without_publishing_frames() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let (mut execution, id) = object_execution(&runtime, context.realm);
    runtime
        .0
        .gc_pressure
        .remaining
        .set(super::LITERAL_ALLOCATION_NODES + 1);
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(matches!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        assert!(state.active_frames.is_empty());
        let frame = execution.frames.current_mut(id).unwrap();
        assert!(!frame.active_frame.is_materialized());
        let Some(JsValue::Object(object)) = execution.pending.take() else {
            panic!("OP_object must complete in the resident executor");
        };
        assert_eq!(state.heap.object_strong_count(object), Ok(1));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
            .unwrap();
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_object_allocations_preserve_nested_backtrace_and_restore_after_throw() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context.eval_with_filename(
            "function outer(){return inner()}\nfunction inner(){let first={};let second={};return fail()}\nfunction fail(){throw new Error('resident')}\nlet correct=false;try{outer()}catch(e){correct=e.stack.includes('at outer (resident-object.js:1:') && e.stack.includes('at inner (resident-object.js:2:') && e.stack.includes('at fail (resident-object.js:3:') && e.stack.split('at outer (').length===2 && e.stack.split('at inner (').length===2} correct",
            "resident-object.js",
        ).unwrap(),
        Value::Bool(true)
    );
    clean(&runtime);
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
}
