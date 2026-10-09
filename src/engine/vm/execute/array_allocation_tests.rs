//! Resident ArrayFrom publishes dense inputs and one result owner before GC.
use crate::engine::{
    api::{Runtime, Value},
    atom::AtomIdx,
    code::{
        bytecode::Instruction,
        function::{UnlinkedFunction, metadata::FunctionMetadata},
    },
    heap::{ContextId, RawId, RawValue},
    value::JsValue,
    vm::{
        BytecodePc,
        call::CallableExecution,
        execution::{ExecutionLimits, RunningExecution},
        frame::{FrameEntry, FrameId},
        frames::{ActiveFrameKind, ActiveFrameToken},
    },
};

fn array_entry(runtime: &Runtime, realm: ContextId, caller: ContextId, count: u16) -> FrameEntry {
    let mut code = vec![Instruction::Undefined; usize::from(count)];
    code.extend([Instruction::ArrayFrom(count), Instruction::Return]);
    let bytecode = runtime
        .publish_unlinked_function(
            realm,
            UnlinkedFunction::fixture(
                code,
                Vec::new(),
                FunctionMetadata {
                    max_stack: count.max(1),
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
    let mut entry = crate::engine::vm::root_call::prepare_call(
        runtime,
        caller,
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

fn array_execution(
    runtime: &Runtime,
    realm: ContextId,
    values: Vec<JsValue>,
) -> (RunningExecution, FrameId, usize) {
    let count = u16::try_from(values.len()).unwrap();
    let entry = array_entry(runtime, realm, realm, count);
    let mut execution = RunningExecution::new(runtime, ExecutionLimits::default()).unwrap();
    let id = crate::engine::vm::driver::push_frame(runtime, &mut execution, entry).unwrap();
    let frame = execution.frames.current_mut(id).unwrap();
    let pc = frame.executable.exec.exec_pc(u32::from(count)).unwrap() as usize;
    frame.resume_pc = pc;
    for value in values {
        execution.slots.push(&mut frame.window, value).unwrap();
    }
    (execution, id, pc)
}

fn clean(runtime: &Runtime) {
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.active_frame_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn resident_array_from_publishes_aliases_symbols_and_fault_metadata_before_pressure_collection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(garbage) = context
        .eval("(()=>{let g={};g.self=g;return g})()")
        .unwrap()
    else {
        panic!("cyclic garbage")
    };
    let garbage = garbage.into_handle();
    runtime.release_jsvalue(JsValue::Object(garbage)).unwrap();
    let object = runtime.new_object(None).unwrap().into_handle();
    runtime.retain_object_handle(object).unwrap();
    let atom = runtime.new_symbol(None).unwrap().into_atom();
    let symbol = AtomIdx::from_raw(atom.raw());
    let (mut execution, id, pc) = array_execution(
        &runtime,
        context.realm,
        vec![
            JsValue::Object(object),
            JsValue::Symbol(symbol),
            JsValue::Object(object),
        ],
    );
    let owners = std::rc::Rc::strong_count(&runtime.0);
    runtime.0.gc_pressure.remaining.set(1);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let array;
    {
        let mut state = runtime.0.state.borrow_mut();
        assert!(state.heap.object(garbage).is_ok());
        assert!(matches!(
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap(),
            super::VmAction::Complete
        ));
        let Some(JsValue::Object(result)) = execution.pending.as_ref() else {
            panic!("resident ArrayFrom result")
        };
        array = *result;
        assert_eq!(state.heap.object_strong_count(array), Ok(1));
        assert_eq!(state.heap.object_strong_count(object), Ok(2));
        assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
        let data = state.heap.object(array).unwrap();
        assert!(matches!(data.dense_array_value(0), Some(RawValue::Object(id)) if *id == object));
        assert!(
            matches!(data.dense_array_value(1), Some(RawValue::Symbol(index)) if *index == symbol)
        );
        assert!(matches!(data.dense_array_value(2), Some(RawValue::Object(id)) if *id == object));
        assert!(matches!(
            data.slots.first(),
            Some(crate::engine::heap::PropertySlot::Data(RawValue::Int(3)))
        ));
        assert!(state.heap.object(garbage).is_err());
        assert!(runtime.0.gc_pressure.remaining.get() > 0);
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        assert!(!runtime.0.deferred_references.has_pending());
        let frame = execution.frames.current_mut(id).unwrap();
        let record = state.active_frames.last().unwrap();
        assert_eq!(record.token, frame.active_frame);
        assert_eq!(record.function, frame.cold.function.object_id());
        assert_eq!(record.realm, context.realm);
        assert_eq!(
            record.kind,
            ActiveFrameKind::Bytecode {
                bytecode: frame.executable.bytecode_id().unwrap(),
                pc: Some(BytecodePc::new(pc)),
            }
        );
    }
    #[cfg(feature = "profiling")]
    {
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events.get("core.internal_array_from"), Some(&1));
        assert_eq!(events.get("core.frame_executor_entry"), Some(&1));
        assert_eq!(events.get("runtime.clone"), None);
        assert_eq!(events.get("runtime.deferred.release"), None);
    }
    drop(execution);
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(array).is_err());
    assert!(state.heap.object(object).is_err());
    assert!(state.atoms.resolve(atom).is_err());
    drop(state);
    clean(&runtime);
}

#[test]
fn resident_array_from_uses_executable_realm_instead_of_caller_realm() {
    let runtime = Runtime::new();
    let first = runtime.new_context().unwrap();
    let second = runtime.new_context().unwrap();
    let entry = array_entry(&runtime, second.realm, first.realm, 0);
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
        let Some(JsValue::Object(array)) = execution.pending.as_ref() else {
            panic!("resident Array result")
        };
        let prototype = state
            .heap
            .shape(state.heap.object(*array).unwrap().shape)
            .unwrap()
            .prototype();
        assert_eq!(
            prototype,
            Some(state.heap.context(second.realm).unwrap().array_prototype)
        );
        assert_ne!(
            prototype,
            Some(state.heap.context(first.realm).unwrap().array_prototype)
        );
        assert_eq!(state.active_frames.last().unwrap().realm, second.realm);
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_array_from_rejected_commit_releases_result_and_keeps_fault_input_and_pressure() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let (mut execution, id, pc) = array_execution(&runtime, context.realm, Vec::new());
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
        let prototype = state.heap.context(context.realm).unwrap().array_prototype;
        let prototype_count = state.heap.object_strong_count(prototype).unwrap();
        let error =
            super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).unwrap_err();
        assert!(
            error
                .message()
                .contains("owned operand stack exceeds verified capacity")
        );
        let after = state.heap.counts();
        assert_eq!(
            (
                after.live,
                after.object_nodes,
                after.shape_nodes,
                after.initializing,
                after.zero_queued
            ),
            (
                before.live,
                before.object_nodes,
                before.shape_nodes,
                before.initializing,
                before.zero_queued
            )
        );
        assert_eq!(
            state.heap.object_strong_count(prototype),
            Ok(prototype_count)
        );
        assert_eq!(state.heap.object_strong_count(marker), Ok(1));
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Object(marker)
        );
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(marker).is_err());
    clean(&runtime);
}

#[test]
fn resident_array_from_allocation_failure_consumes_inputs_without_advance_or_collection() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let warm = runtime.new_array(context.realm).unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let last = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id, pc) = array_execution(
        &runtime,
        context.realm,
        vec![JsValue::Object(first), JsValue::Object(last)],
    );
    // A latched request ignores RC credits and must not be serviced on error.
    runtime.0.gc_pressure.remaining.set(0);
    {
        let mut state = runtime.0.state.borrow_mut();
        let shape = state.heap.object(warm.object_id()).unwrap().shape;
        assert!(state.shape_is_canonical(shape));
        let prototype = state.heap.context(context.realm).unwrap().array_prototype;
        let count = state.heap.object_strong_count(prototype).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
        let result = super::execute_frame_in_state(&runtime, &mut state, &mut execution, id);
        state
            .heap
            .set_strong_count_for_test(RawId::Object(prototype), count);
        assert!(result.is_err());
        assert!(state.heap.object(first).is_err());
        assert!(state.heap.object(last).is_err());
        assert_eq!(runtime.0.gc_pressure.remaining.get(), 0);
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(execution.slots.depth(&frame.window), 0);
        assert!(execution.pending.is_none());
        assert!(!runtime.is_poisoned());
    }
    drop(execution);
    clean(&runtime);
}

#[test]
fn resident_array_from_materialization_failure_leaves_inputs_and_fault_in_frame() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let last = runtime.new_object(None).unwrap().into_handle();
    let (mut execution, id, pc) = array_execution(
        &runtime,
        context.realm,
        vec![JsValue::Object(first), JsValue::Object(last)],
    );
    // Near a collection the frames are published before allocating.
    runtime.0.gc_pressure.remaining.set(1);
    {
        let mut state = runtime.0.state.borrow_mut();
        state.next_active_frame_token = u64::MAX;
        let before = state.heap.counts();
        assert!(super::execute_frame_in_state(&runtime, &mut state, &mut execution, id).is_err());
        assert_eq!(state.heap.counts(), before);
        assert!(state.active_frames.is_empty());
        assert_eq!(state.heap.object_strong_count(first), Ok(1));
        assert_eq!(state.heap.object_strong_count(last), Ok(1));
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!((frame.fault_pc, frame.resume_pc), (pc, pc));
        assert_eq!(execution.slots.depth(&frame.window), 2);
        assert_eq!(
            execution.slots.peek(&frame.window, 0).unwrap(),
            &JsValue::Object(last)
        );
    }
    drop(execution);
    assert!(runtime.0.state.borrow().heap.object(first).is_err());
    assert!(runtime.0.state.borrow().heap.object(last).is_err());
    clean(&runtime);
}

#[test]
fn resident_array_literals_keep_holes_spreads_evaluation_order_and_nested_backtrace() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval_with_filename(
        "function outer(){return inner()}\nfunction inner(){let log=[];function item(v){log.push(v);return v} let a=[item(1),item(2),...[,item(3)]];let b=[,];if(a.join(',')!=='1,2,,3'||log.join(',')!=='1,2,3'||0 in b)throw Error('literal');return fail(a)}\nfunction fail(a){throw new Error(a.length)}\nlet ok=false;try{outer()}catch(e){ok=e.message==='4'&&e.stack.includes('at outer (resident-array.js:1:')&&e.stack.includes('at inner (resident-array.js:2:')&&e.stack.includes('at fail (resident-array.js:3:')&&e.stack.split('at outer (').length===2&&e.stack.split('at inner (').length===2}ok",
        "resident-array.js").unwrap(), Value::Bool(true));
    clean(&runtime);
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
}
