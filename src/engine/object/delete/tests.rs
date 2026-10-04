use super::*;
use crate::engine::{
    heap::{RawId, RawValue},
    value::Value,
};
fn slot(state: &RuntimeState, object: ObjectId, atom: Atom) -> Option<PropertySlot> {
    let object = state.heap.object(object).unwrap();
    let index = state
        .heap
        .shape(object.shape)
        .unwrap()
        .find(AtomIdx::from_raw(atom.raw()))?;
    object.slots.get(index as usize).cloned()
}
fn global() -> (Runtime, ObjectRef, ObjectRef, ObjectRef) {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let hidden = runtime.new_object(None).unwrap();
    let object = runtime.new_global_object(&prototype, &hidden).unwrap();
    (runtime, prototype, hidden, object)
}
#[test]
fn public_delete_operation_domain_precedence_remains_unchanged() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let object = foreign.new_object(None).unwrap();
    let key = runtime.intern_property_key("foreign deletion key").unwrap();
    assert!(matches!(
        runtime.delete_property(&object, &key),
        Err(RuntimeError::WrongRuntime("object"))
    ));
    assert!(!runtime.is_poisoned());
}
#[test]
fn ordinary_dictionary_delete_preserves_policy_and_does_not_invoke_accessor() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("var deletedReads=0; var deletedObject={a:1,b:2}; Object.defineProperty(deletedObject,'long_deletion_accessor',{get(){deletedReads++;return 3},configurable:true}); delete deletedObject.long_deletion_accessor && deletedReads===0 && deletedObject.a===1 && deletedObject.b===2").unwrap(),Value::Bool(true));
}
#[test]
fn dense_tail_and_interior_delete_keep_logical_length_and_one_materialization() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(tail) = context.eval("[1,2,3]").unwrap() else {
        panic!("array")
    };
    let Value::Object(interior) = context.eval("[1,2,3]").unwrap() else {
        panic!("array")
    };
    let two = runtime.property_key_for_index(2).unwrap();
    let one = runtime.property_key_for_index(1).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    assert!(matches!(
        state
            .delete_own_property_in_state(&runtime.0.poisoned, tail.object_id(), two.atom())
            .unwrap(),
        DeleteOwnProperty::Complete(true)
    ));
    assert_eq!(state.heap.array_dense_len(tail.object_id()), Ok(Some(2)));
    assert_eq!(state.array_length_state(tail.object_id()).unwrap().0, 3);
    assert!(matches!(
        state
            .delete_own_property_in_state(&runtime.0.poisoned, interior.object_id(), one.atom())
            .unwrap(),
        DeleteOwnProperty::Complete(true)
    ));
    assert_eq!(state.heap.array_dense_len(interior.object_id()), Ok(None));
    assert_eq!(state.array_length_state(interior.object_id()).unwrap().0, 3);
    assert!(slot(&state, interior.object_id(), one.atom()).is_none());
}
#[test]
fn typed_and_string_virtual_delete_use_the_canonical_selected_dependencies() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("var deletionTyped=new Uint8Array([1,2]); var deletionString=new String('ab'); !Reflect.deleteProperty(deletionTyped,'0') && Reflect.deleteProperty(deletionTyped,'5') && Reflect.deleteProperty(deletionTyped,'-0') && !Reflect.deleteProperty(deletionString,'0') && Reflect.deleteProperty(deletionString,'5')").unwrap(),Value::Bool(true));
}
#[test]
fn global_delete_moves_shared_cell_to_hidden_then_resets_metadata_before_slot_delete() {
    let (runtime, _prototype, hidden, object) = global();
    let key = runtime
        .intern_property_key("shared deleted global cell")
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Int(7),
            true,
            true,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state
        .store_property_slot_with_poison(
            &runtime.0.poisoned,
            object.object_id(),
            key.atom(),
            PropertyFlags::data(true, true, true),
            PropertySlot::VarRef(cell),
        )
        .unwrap();
    let hidden_count = state.heap.object_strong_count(hidden.object_id()).unwrap();
    assert!(matches!(
        state
            .delete_own_property_in_state(&runtime.0.poisoned, object.object_id(), key.atom())
            .unwrap(),
        DeleteOwnProperty::Complete(true)
    ));
    assert!(slot(&state, object.object_id(), key.atom()).is_none());
    assert!(
        matches!(slot(&state,hidden.object_id(),key.atom()),Some(PropertySlot::VarRef(id))if id==cell)
    );
    let data = state.heap.var_ref(cell).unwrap();
    assert!(matches!(data.value, RawValue::Uninitialized));
    assert!(!data.is_lexical);
    assert!(!data.is_const);
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    assert_eq!(
        state.heap.object_strong_count(hidden.object_id()),
        Ok(hidden_count)
    );
    state.release_var_ref_handle(cell).unwrap();
}
#[test]
fn global_hidden_temporary_overflow_is_recoverable_before_any_publication() {
    let (runtime, _prototype, hidden, object) = global();
    let key = runtime
        .intern_property_key("global delete hidden overflow")
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Int(7),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state
        .store_property_slot_with_poison(
            &runtime.0.poisoned,
            object.object_id(),
            key.atom(),
            PropertyFlags::data(true, true, true),
            PropertySlot::VarRef(cell),
        )
        .unwrap();
    let before = state.heap.object_strong_count(hidden.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(hidden.object_id()), u32::MAX);
    assert!(
        state
            .delete_own_property_in_state(&runtime.0.poisoned, object.object_id(), key.atom())
            .is_err()
    );
    assert!(!runtime.is_poisoned());
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    assert!(
        matches!(slot(&state,object.object_id(),key.atom()),Some(PropertySlot::VarRef(id))if id==cell)
    );
    assert!(slot(&state, hidden.object_id(), key.atom()).is_none());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(hidden.object_id()), before);
    state.release_var_ref_handle(cell).unwrap();
}
#[test]
fn global_reset_destructive_failure_stops_before_main_slot_deletion() {
    let (runtime, _prototype, _hidden, object) = global();
    let key = runtime
        .intern_property_key("global delete first fatal")
        .unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Int(7),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state
        .store_property_slot_with_poison(
            &runtime.0.poisoned,
            object.object_id(),
            key.atom(),
            PropertyFlags::data(true, true, true),
            PropertySlot::VarRef(cell),
        )
        .unwrap();
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
    assert!(
        state
            .delete_own_property_in_state(&runtime.0.poisoned, object.object_id(), key.atom())
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert!(
        matches!(slot(&state,object.object_id(),key.atom()),Some(PropertySlot::VarRef(id))if id==cell)
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn shared_typed_delete_carries_exact_word_seed_without_a_temporary_object_owner() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(object) = context
        .eval("new Uint8Array(new SharedArrayBuffer(4))")
        .unwrap()
    else {
        panic!("shared view")
    };
    let key = runtime.property_key_for_index(0).unwrap();
    let before = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(object.object_id())
        .unwrap();
    let selected = runtime
        .0
        .state
        .borrow_mut()
        .delete_own_property_in_state(&runtime.0.poisoned, object.object_id(), key.atom())
        .unwrap();
    assert!(matches!(selected, DeleteOwnProperty::Shared(_)));
    assert!(!selected.finish_shared().unwrap());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(object.object_id()),
        Ok(before)
    );
}
