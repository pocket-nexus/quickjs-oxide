//! Real local instruction inputs before prototype preparation and handoff.
use super::*;
use crate::engine::{
    api::Value, code::exec_opcode::Opcode, heap::RawId,
    vm::property_driver::read_completion_tests::read_fixture,
};

fn stale_realm(runtime: &Runtime) -> crate::engine::heap::ContextId {
    let context = runtime.new_context().unwrap();
    let realm = context.realm;
    drop(context);
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().heap.context(realm).is_err());
    realm
}

#[test]
fn local_guard_retires_value_then_primitive_receiver_on_prototype_preparation_error() {
    let runtime = Runtime::new();
    let realm = stale_realm(&runtime);
    let mut context = runtime.new_context().unwrap();
    let value = runtime.new_object(None).unwrap().into_handle();
    let receiver = runtime
        .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
            "owned local receiver",
        )))
        .unwrap();
    let JsValue::String(receiver_id) = receiver else {
        unreachable!()
    };
    let property = runtime.intern_property_key("x").unwrap();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,v){o.x=v})",
        Opcode::PutField,
    );
    let frame = execution.frames.current_mut(id).unwrap();
    let pc = frame.resume_pc;
    // execute_frame_in_state publishes this fault PC before its write arm.
    frame.fault_pc = pc;
    let mut segment = FrameExecution::admit(&mut execution, id).unwrap();
    {
        let mut turn = segment.frame();
        turn.transaction.slots().push(JsValue::Int(99)).unwrap();
        turn.transaction
            .slots()
            .push(JsValue::String(receiver_id))
            .unwrap();
        turn.transaction
            .slots()
            .push(JsValue::Object(value))
            .unwrap();
    }
    // Use the real local consumer with a reclaimed realm: primitive prototype
    // preparation fails before either input transfers to the Set selector.
    let depth = segment.frame().transaction.depth();
    let fallthrough = FallthroughPc::from_committed_index(pc).unwrap();
    let guards = RawNativeQuery::guard_constructions_for_test();
    let mut state = runtime.0.state.borrow_mut();
    let result = run_local_assignment(
        &runtime,
        &mut state,
        &mut segment,
        realm,
        property.atom(),
        false,
        fallthrough,
        depth,
        false,
        &mut None,
    );
    assert!(result.is_err());
    assert!(!runtime.is_poisoned());
    assert!(state.heap.object(value).is_err());
    assert!(state.heap.string(receiver_id).is_err());
    assert_eq!(RawNativeQuery::guard_constructions_for_test(), guards);
    let turn = segment.frame();
    assert_eq!((*turn.fault_pc, *turn.resume_pc), (pc, pc));
    assert_eq!(turn.transaction.depth(), 1);
    assert_eq!(turn.transaction.peek(0).unwrap(), &JsValue::Int(99));
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn local_guard_fatal_preselection_retirement_stops_receiver_and_computed_atom_suffix() {
    let runtime = Runtime::new();
    let realm = stale_realm(&runtime);
    let mut context = runtime.new_context().unwrap();
    let Value::Object(parent) = context.eval("({child:{}})").unwrap() else {
        unreachable!()
    };
    let child_key = runtime.intern_property_key("child").unwrap();
    let Value::Object(child) = context.get_property(&parent, &child_key).unwrap() else {
        unreachable!()
    };
    let child_id = child.object_id();
    drop(child);
    let parent = parent.into_handle();
    let receiver = runtime
        .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
            "quarantined local receiver",
        )))
        .unwrap();
    let JsValue::String(receiver) = receiver else {
        unreachable!()
    };
    let property = runtime
        .intern_property_key("local_guard_atom_suffix")
        .unwrap();
    let (mut execution, id) = read_fixture(
        &runtime,
        &mut context,
        "(function(o,k,v){o[k]=v})",
        Opcode::PutArrayEl,
    );
    let frame = execution.frames.current_mut(id).unwrap();
    let pc = frame.resume_pc;
    // execute_frame_in_state publishes this fault PC before its write arm.
    frame.fault_pc = pc;
    let mut segment = FrameExecution::admit(&mut execution, id).unwrap();
    {
        let mut turn = segment.frame();
        turn.transaction.slots().push(JsValue::Int(99)).unwrap();
        turn.transaction
            .slots()
            .push(JsValue::String(receiver))
            .unwrap();
        turn.transaction.slots().push(JsValue::Int(0)).unwrap();
        turn.transaction
            .slots()
            .push(JsValue::Object(parent))
            .unwrap();
    }
    let depth = segment.frame().transaction.depth();
    let fallthrough = FallthroughPc::from_committed_index(pc).unwrap();
    let guards = RawNativeQuery::guard_constructions_for_test();
    let mut state = runtime.0.state.borrow_mut();
    let count = state
        .atoms
        .resolve(property.atom())
        .unwrap()
        .ref_count
        .unwrap();
    let owned_atom = state.atoms.retain(property.atom()).unwrap();
    let mut atom_guard = OwnedValueGuard::new(
        &mut state,
        &runtime.0.poisoned,
        JsValue::Symbol(crate::engine::atom::AtomIdx::from_raw(owned_atom.raw())),
    );
    let (state, atom_owner) = atom_guard.parts();
    // The real parent release starts successfully, then fails on its actual
    // child edge. The primitive receiver and new atom are later raw roles.
    state
        .heap
        .set_strong_count_for_test(RawId::Object(child_id), 0);
    let result = run_local_assignment(
        &runtime,
        state,
        &mut segment,
        realm,
        owned_atom,
        false,
        fallthrough,
        depth,
        true,
        atom_owner,
    );
    assert!(
        matches!(result, Err(error) if error.clone().into_runtime_error() == RuntimeError::Poisoned)
    );
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.strong_count(RawId::String(receiver)), Ok(1));
    assert_eq!(
        state.atoms.resolve(property.atom()).unwrap().ref_count,
        Some(count + 1)
    );
    assert!(atom_owner.is_some());
    assert_eq!(RawNativeQuery::guard_constructions_for_test(), guards);
    let turn = segment.frame();
    assert_eq!((*turn.fault_pc, *turn.resume_pc), (pc, pc));
    assert_eq!(turn.transaction.depth(), 1);
    assert_eq!(turn.transaction.peek(0).unwrap(), &JsValue::Int(99));
}
