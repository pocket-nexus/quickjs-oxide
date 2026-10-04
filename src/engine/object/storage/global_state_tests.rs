use super::*;
use crate::engine::heap::{Heap, HeapError, ObjectData, RawId, Slots};
use crate::engine::object::property::{PropertyDefinitionError, PropertyDescriptor};
use crate::engine::object::shape::Shape;

fn fixture() -> (Runtime, ObjectRef, ObjectRef, ObjectRef) {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let hidden = runtime.new_object(None).unwrap();
    let global = runtime.new_global_object(&prototype, &hidden).unwrap();
    (runtime, prototype, hidden, global)
}

fn data(value: RawValue) -> CompletePropertyDescriptor<RawValue> {
    CompletePropertyDescriptor::Data {
        value,
        writable: true,
        enumerable: true,
        configurable: true,
    }
}

fn accessor() -> CompletePropertyDescriptor<RawValue> {
    CompletePropertyDescriptor::Accessor {
        get: None,
        set: None,
        enumerable: true,
        configurable: true,
    }
}

fn slot(state: &RuntimeState, object: ObjectId, atom: Atom) -> Option<PropertySlot> {
    let object = state.heap.object(object).unwrap();
    state
        .heap
        .shape(object.shape)
        .unwrap()
        .find(AtomIdx::from_raw(atom.raw()))
        .map(|index| object.slots[index as usize].clone())
}

// Return a producer-owned cell in addition to its hidden-table edge.
fn hidden_cell(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    hidden: ObjectId,
    atom: Atom,
    configurable: bool,
) -> VarRefId {
    let cell = state
        .new_var_ref(
            poisoned,
            JsValue::Int(7),
            true,
            true,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    state
        .store_property_slot(
            hidden,
            atom,
            PropertyFlags::data(true, true, configurable),
            PropertySlot::VarRef(cell),
        )
        .unwrap();
    cell
}

fn assert_raw(actual: &RawValue, expected: &RawValue) {
    match (actual, expected) {
        (RawValue::Int(actual), RawValue::Int(expected)) => assert_eq!(actual, expected),
        (RawValue::Symbol(actual), RawValue::Symbol(expected)) => assert_eq!(actual, expected),
        (RawValue::Object(actual), RawValue::Object(expected)) => assert_eq!(actual, expected),
        (RawValue::Uninitialized, RawValue::Uninitialized) => {}
        _ => panic!("raw representation mismatch: {actual:?}, expected {expected:?}"),
    }
}

fn assert_slot(actual: Option<PropertySlot>, expected: Option<PropertySlot>) {
    match (&actual, &expected) {
        (None, None) => {}
        (Some(PropertySlot::VarRef(actual)), Some(PropertySlot::VarRef(expected))) => {
            assert_eq!(actual, expected)
        }
        (Some(PropertySlot::Data(actual)), Some(PropertySlot::Data(expected))) => {
            assert_raw(actual, expected)
        }
        (
            Some(PropertySlot::Accessor {
                get: actual_get,
                set: actual_set,
            }),
            Some(PropertySlot::Accessor {
                get: expected_get,
                set: expected_set,
            }),
        ) => {
            assert_eq!(actual_get.option(), expected_get.option());
            assert_eq!(actual_set.option(), expected_set.option());
        }
        _ => panic!("slot representation mismatch: {actual:?}, expected {expected:?}"),
    }
}

fn queue_error() -> RuntimeError {
    RuntimeError::Heap(HeapError::Invariant(
        "finalization count disagrees with its queue/cycle state",
    ))
}

#[test]
fn global_state_data_preserves_cell_identity_checked_edges_and_metadata() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("direct-cell").unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let hidden_before = state.heap.object_strong_count(hidden.object_id()).unwrap();
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Int(7)),
        )
        .unwrap();
    let cell = state
        .var_ref_property(global.object_id(), key.atom())
        .unwrap()
        .unwrap();
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(1));
    state.heap.retain_var_ref(cell).unwrap();
    let atom = state.atoms.new_symbol(Some("stored-symbol")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            CompletePropertyDescriptor::Data {
                value: RawValue::Symbol(index),
                writable: false,
                enumerable: false,
                configurable: true,
            },
        )
        .unwrap();
    assert_slot(
        slot(&state, global.object_id(), key.atom()),
        Some(PropertySlot::VarRef(cell)),
    );
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    let metadata = state.heap.var_ref(cell).unwrap();
    assert!(!metadata.is_lexical);
    assert!(metadata.is_const);
    assert_eq!(metadata.kind, ClosureVariableKind::Normal);
    assert_raw(&metadata.value, &RawValue::Symbol(index));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(2));
    assert_eq!(
        state.heap.object_strong_count(hidden.object_id()),
        Ok(hidden_before)
    );
    state.release_var_ref_handle(cell).unwrap();
    state.release_atom_index(index).unwrap();
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn global_state_unshared_accessor_retires_cell_without_hidden_binding() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("unshared-cell").unwrap();
    let value = runtime.new_object(None).unwrap();
    let value_id = value.object_id();
    {
        let mut state = runtime.0.state.borrow_mut();
        state
            .store_complete_global_raw_property(
                &runtime.0.poisoned,
                global.object_id(),
                hidden.object_id(),
                key.atom(),
                data(RawValue::Object(value_id)),
            )
            .unwrap();
    }
    drop(value);
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = state
        .var_ref_property(global.object_id(), key.atom())
        .unwrap()
        .unwrap();
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(1));
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            accessor(),
        )
        .unwrap();
    assert!(state.heap.var_ref(cell).is_err());
    assert!(state.heap.object(value_id).is_err());
    assert_slot(slot(&state, hidden.object_id(), key.atom()), None);
    assert_slot(
        slot(&state, global.object_id(), key.atom()),
        Some(PropertySlot::accessor(None, None)),
    );
}

#[test]
fn global_state_shared_accessor_resets_then_hidden_data_reconnects_same_cell() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("shared-cell").unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Int(1)),
        )
        .unwrap();
    let cell = state
        .var_ref_property(global.object_id(), key.atom())
        .unwrap()
        .unwrap();
    state.heap.retain_var_ref(cell).unwrap();
    state
        .set_var_ref_metadata(cell, true, true, ClosureVariableKind::Normal)
        .unwrap();
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            accessor(),
        )
        .unwrap();
    assert_slot(
        slot(&state, hidden.object_id(), key.atom()),
        Some(PropertySlot::VarRef(cell)),
    );
    let metadata = state.heap.var_ref(cell).unwrap();
    assert_raw(&metadata.value, &RawValue::Uninitialized);
    assert!(!metadata.is_lexical && !metadata.is_const);
    assert_eq!(metadata.kind, ClosureVariableKind::Normal);
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    let hidden_data = state.heap.object(hidden.object_id()).unwrap();
    let flags = state.heap.shape(hidden_data.shape).unwrap().entries()[0].flags;
    assert_eq!(flags, PropertyFlags::data(true, true, true));
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Int(12)),
        )
        .unwrap();
    assert_slot(slot(&state, hidden.object_id(), key.atom()), None);
    assert_slot(
        slot(&state, global.object_id(), key.atom()),
        Some(PropertySlot::VarRef(cell)),
    );
    assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(12));
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    state.release_var_ref_handle(cell).unwrap();
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn global_state_hidden_object_and_cell_temporary_overflow_stay_recoverable() {
    for hidden_overflow in [true, false] {
        let (runtime, _prototype, hidden, global) = fixture();
        let key = runtime.intern_property_key("retain-order").unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        state
            .store_complete_global_raw_property(
                &runtime.0.poisoned,
                global.object_id(),
                hidden.object_id(),
                key.atom(),
                data(RawValue::Int(7)),
            )
            .unwrap();
        let cell = state
            .var_ref_property(global.object_id(), key.atom())
            .unwrap()
            .unwrap();
        let hidden_before = state.heap.object_strong_count(hidden.object_id()).unwrap();
        let target = if hidden_overflow {
            RawId::Object(hidden.object_id())
        } else {
            RawId::VarRef(cell)
        };
        let before = state.heap.strong_count(target).unwrap();
        state.heap.set_strong_count_for_test(target, u32::MAX);
        let result = state.store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Uninitialized),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert_eq!(state.heap.strong_count(target), Ok(u32::MAX));
        state.heap.set_strong_count_for_test(target, before);
        assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(7));
        assert_eq!(state.heap.var_ref_strong_count(cell), Ok(1));
        assert_eq!(
            state.heap.object_strong_count(hidden.object_id()),
            Ok(hidden_before)
        );
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn global_state_hidden_delete_precedes_input_retain_failure() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("hidden-to-global").unwrap();
    let input = runtime.new_object(None).unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = hidden_cell(
        &mut state,
        &runtime.0.poisoned,
        hidden.object_id(),
        key.atom(),
        true,
    );
    let before = state.heap.object_strong_count(input.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(input.object_id()), u32::MAX);
    let result = state.store_complete_global_raw_property(
        &runtime.0.poisoned,
        global.object_id(),
        hidden.object_id(),
        key.atom(),
        data(RawValue::Object(input.object_id())),
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(input.object_id()), before);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_slot(slot(&state, hidden.object_id(), key.atom()), None);
    assert_slot(slot(&state, global.object_id(), key.atom()), None);
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(1));
    assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(7));
    state.release_var_ref_handle(cell).unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn global_state_fixed_hidden_slot_fails_before_internal_input_validation() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("fixed-hidden").unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = hidden_cell(
        &mut state,
        &runtime.0.poisoned,
        hidden.object_id(),
        key.atom(),
        false,
    );
    assert_eq!(
        state.store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Uninitialized),
        ),
        Err(RuntimeError::Invariant(
            "hidden global VarRef property was not configurable"
        ))
    );
    assert_slot(
        slot(&state, hidden.object_id(), key.atom()),
        Some(PropertySlot::VarRef(cell)),
    );
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(7));
    state.release_var_ref_handle(cell).unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn global_state_hidden_probe_retain_and_retirement_precede_mismatch_error() {
    for probe_overflow in [false, true] {
        let (runtime, _prototype, hidden, global) = fixture();
        let key = runtime.intern_property_key("conflicting-hidden").unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        state
            .store_complete_global_raw_property(
                &runtime.0.poisoned,
                global.object_id(),
                hidden.object_id(),
                key.atom(),
                data(RawValue::Int(8)),
            )
            .unwrap();
        let cell = state
            .var_ref_property(global.object_id(), key.atom())
            .unwrap()
            .unwrap();
        state.heap.retain_var_ref(cell).unwrap();
        let other = hidden_cell(
            &mut state,
            &runtime.0.poisoned,
            hidden.object_id(),
            key.atom(),
            true,
        );
        if probe_overflow {
            state
                .heap
                .set_strong_count_for_test(RawId::VarRef(other), u32::MAX);
        }
        let result = state.store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            accessor(),
        );
        if probe_overflow {
            assert!(matches!(
                result,
                Err(RuntimeError::Heap(HeapError::Overflow { .. }))
            ));
            assert_eq!(state.heap.var_ref_strong_count(other), Ok(u32::MAX));
            state
                .heap
                .set_strong_count_for_test(RawId::VarRef(other), 2);
        } else {
            assert_eq!(
                result,
                Err(RuntimeError::Invariant(
                    "global property and hidden table contain distinct VarRefs"
                ))
            );
            assert_eq!(state.heap.var_ref_strong_count(other), Ok(2));
        }
        assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
        assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(8));
        assert_raw(&state.heap.var_ref(other).unwrap().value, &RawValue::Int(7));
        state.release_var_ref_handle(cell).unwrap();
        state.release_var_ref_handle(other).unwrap();
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn global_state_accessor_edges_preserve_alias_counts_and_second_overflow_rollback() {
    let (runtime, _prototype, hidden, global) = fixture();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(getter) = context.eval("(function(){return 1})").unwrap() else {
        panic!("getter")
    };
    let Value::Object(setter) = context.eval("(function(v){})").unwrap() else {
        panic!("setter")
    };
    let key = runtime.intern_property_key("accessor-edges").unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let getter_before = state.heap.object_strong_count(getter.object_id()).unwrap();
    let setter_before = state.heap.object_strong_count(setter.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
    let result = state.store_complete_global_raw_property(
        &runtime.0.poisoned,
        global.object_id(),
        hidden.object_id(),
        key.atom(),
        CompletePropertyDescriptor::Accessor {
            get: Some(RawValue::Object(getter.object_id())),
            set: Some(RawValue::Object(setter.object_id())),
            enumerable: true,
            configurable: true,
        },
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), setter_before);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(
        state.heap.object_strong_count(getter.object_id()),
        Ok(getter_before)
    );
    assert_slot(slot(&state, global.object_id(), key.atom()), None);
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            CompletePropertyDescriptor::Accessor {
                get: Some(RawValue::Object(getter.object_id())),
                set: Some(RawValue::Object(getter.object_id())),
                enumerable: true,
                configurable: true,
            },
        )
        .unwrap();
    assert_eq!(
        state.heap.object_strong_count(getter.object_id()),
        Ok(getter_before + 2)
    );
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Object(global.object_id())),
        )
        .unwrap();
    assert_eq!(
        state.heap.object_strong_count(getter.object_id()),
        Ok(getter_before)
    );
    assert_eq!(state.heap.object_strong_count(global.object_id()), Ok(2));
    // Remove the self edge before ordinary fixture teardown.
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Int(42)),
        )
        .unwrap();
    assert_eq!(state.heap.object_strong_count(global.object_id()), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn global_raw_validation_precedes_hidden_temporary_retain() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("fixed-global").unwrap();
    let hidden_before = {
        let mut state = runtime.0.state.borrow_mut();
        state
            .store_complete_global_raw_property(
                &runtime.0.poisoned,
                global.object_id(),
                hidden.object_id(),
                key.atom(),
                CompletePropertyDescriptor::Data {
                    value: RawValue::Int(7),
                    writable: false,
                    enumerable: false,
                    configurable: false,
                },
            )
            .unwrap();
        let before = state.heap.object_strong_count(hidden.object_id()).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(hidden.object_id()), u32::MAX);
        before
    };
    let rejected = runtime.define_raw_property(
        &global,
        &key,
        &PropertyDescriptor {
            value: Some(RawValue::Int(8)),
            ..PropertyDescriptor::new()
        },
    );
    let invalid = runtime.define_raw_property(
        &global,
        &key,
        &PropertyDescriptor {
            value: Some(RawValue::Int(7)),
            get: Some(None),
            ..PropertyDescriptor::new()
        },
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(hidden.object_id()), hidden_before);
    assert_eq!(rejected, Ok(false));
    assert_eq!(
        invalid,
        Err(RuntimeError::Property(
            PropertyDefinitionError::InvalidDescriptor
        ))
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn global_state_accessor_retirement_failure_stops_publication_and_suffix_cleanup() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("retirement-failure").unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Int(7)),
        )
        .unwrap();
    let cell = state
        .var_ref_property(global.object_id(), key.atom())
        .unwrap()
        .unwrap();
    let hidden_before = state.heap.object_strong_count(hidden.object_id()).unwrap();
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
    assert_eq!(
        state.store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            accessor(),
        ),
        Err(queue_error())
    );
    assert!(runtime.is_poisoned());
    assert_slot(
        slot(&state, global.object_id(), key.atom()),
        Some(PropertySlot::VarRef(cell)),
    );
    assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(7));
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(1));
    assert_eq!(
        state.heap.object_strong_count(hidden.object_id()),
        Ok(hidden_before + 1)
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn global_state_published_layout_failure_quarantines_before_temporary_retirement() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("layout-failure").unwrap();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(getter) = context.eval("(function(){return 1})").unwrap() else {
        panic!("getter")
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            CompletePropertyDescriptor::Accessor {
                get: Some(RawValue::Object(getter.object_id())),
                set: None,
                enumerable: true,
                configurable: true,
            },
        )
        .unwrap();
    let hidden_before = state.heap.object_strong_count(hidden.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(getter.object_id()), 0);
    assert!(
        state
            .store_complete_global_raw_property(
                &runtime.0.poisoned,
                global.object_id(),
                hidden.object_id(),
                key.atom(),
                data(RawValue::Int(9)),
            )
            .is_err()
    );
    assert!(runtime.is_poisoned());
    let cell = state
        .var_ref_property(global.object_id(), key.atom())
        .unwrap()
        .unwrap();
    assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(9));
    assert_eq!(
        state.heap.var_ref_strong_count(cell),
        Ok(2),
        "slot and abandoned temporary remain owned"
    );
    assert_eq!(
        state.heap.object_strong_count(hidden.object_id()),
        Ok(hidden_before + 1)
    );
}

#[test]
fn global_state_hidden_delete_failure_is_published_before_input_duplication() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("delete-failure").unwrap();
    let input = runtime.new_object(None).unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let cell = hidden_cell(
        &mut state,
        &runtime.0.poisoned,
        hidden.object_id(),
        key.atom(),
        true,
    );
    let hidden_before = state.heap.object_strong_count(hidden.object_id()).unwrap();
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
    assert_eq!(
        state.store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Object(input.object_id())),
        ),
        Err(queue_error())
    );
    assert!(runtime.is_poisoned());
    assert_slot(slot(&state, hidden.object_id(), key.atom()), None);
    assert_slot(slot(&state, global.object_id(), key.atom()), None);
    assert_eq!(
        state.heap.var_ref_strong_count(cell),
        Ok(2),
        "producer and abandoned temporary remain owned"
    );
    assert_raw(&state.heap.var_ref(cell).unwrap().value, &RawValue::Int(7));
    assert_eq!(state.heap.object_strong_count(input.object_id()), Ok(1));
    assert_eq!(
        state.heap.object_strong_count(hidden.object_id()),
        Ok(hidden_before + 1)
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn global_state_accessor_reset_drains_older_queue_before_detached_symbol() {
    let (runtime, _prototype, hidden, global) = fixture();
    let key = runtime.intern_property_key("reset-order").unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let atom = state.atoms.new_symbol(Some("reset-detached")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    state
        .store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            data(RawValue::Symbol(index)),
        )
        .unwrap();
    let cell = state
        .var_ref_property(global.object_id(), key.atom())
        .unwrap()
        .unwrap();
    state.heap.retain_var_ref(cell).unwrap();
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
    assert_eq!(
        state.store_complete_global_raw_property(
            &runtime.0.poisoned,
            global.object_id(),
            hidden.object_id(),
            key.atom(),
            accessor(),
        ),
        Err(queue_error())
    );
    assert!(runtime.is_poisoned());
    assert_raw(
        &state.heap.var_ref(cell).unwrap().value,
        &RawValue::Uninitialized,
    );
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(3));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(2));
    assert_slot(slot(&state, hidden.object_id(), key.atom()), None);
    assert_slot(
        slot(&state, global.object_id(), key.atom()),
        Some(PropertySlot::VarRef(cell)),
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn global_descriptor_binding_eval_alias_and_delete_reconnect_remain_live() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(r#"
        globalThis.stageCell = 1;
        function stageRead() { return stageCell; }
        Object.defineProperty(globalThis, 'stageCell', {value: 2, configurable: true});
        if (stageRead() !== 2) throw new Error('data alias');
        Object.defineProperty(globalThis, 'stageCell', {get() { return 33; }, configurable: true});
        if (stageRead() !== 33) throw new Error('accessor alias');
        delete globalThis.stageCell;
        let missing = false;
        try { stageRead(); } catch (e) { missing = e instanceof ReferenceError; }
        if (!missing) throw new Error('deleted alias');
        Object.defineProperty(globalThis, 'stageCell', {value: 44, writable: true, configurable: true});
        if (stageRead() !== 44) throw new Error('reconnected alias');
        let stageLexical = 5;
        globalThis.stageLexical = 19;
        Object.defineProperty(globalThis, 'stageLexical', {value: 23});
        stageLexical === 5 && globalThis.stageLexical === 23 && eval('stageCell') === 44
    "#).unwrap(), Value::Bool(true));
}

#[test]
fn state_poison_descriptor_entry_shares_validation_and_self_edge_ownership() {
    let runtime = Runtime::new();
    let owner = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("poison-aware-raw").unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.object_strong_count(owner.object_id()).unwrap();
    assert_eq!(
        state.define_raw_property_with_poison(
            &runtime.0.poisoned,
            owner.object_id(),
            key.atom(),
            &PropertyDescriptor {
                value: Some(RawValue::Object(owner.object_id())),
                writable: Some(true),
                configurable: Some(true),
                ..PropertyDescriptor::new()
            },
        ),
        Ok(true)
    );
    assert_eq!(
        state.heap.object_strong_count(owner.object_id()),
        Ok(before + 1)
    );
    assert_eq!(
        state.define_raw_property_with_poison(
            &runtime.0.poisoned,
            owner.object_id(),
            key.atom(),
            &PropertyDescriptor {
                value: Some(RawValue::Int(9)),
                get: Some(None),
                ..PropertyDescriptor::new()
            },
        ),
        Err(RuntimeError::Property(
            PropertyDefinitionError::InvalidDescriptor
        ))
    );
    assert!(!runtime.is_poisoned());
    assert_eq!(
        state.define_raw_property_with_poison(
            &runtime.0.poisoned,
            owner.object_id(),
            key.atom(),
            &PropertyDescriptor {
                value: Some(RawValue::Int(9)),
                ..PropertyDescriptor::new()
            },
        ),
        Ok(true)
    );
    assert_eq!(
        state.heap.object_strong_count(owner.object_id()),
        Ok(before)
    );
    assert_slot(
        slot(&state, owner.object_id(), key.atom()),
        Some(PropertySlot::Data(RawValue::Int(9))),
    );
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn state_poison_descriptor_entry_preserves_published_atom_before_owner_drop() {
    for dictionary in [false, true] {
        let runtime = Runtime::new();
        let owner = runtime.new_object(None).unwrap();
        let mut context = runtime.new_context().unwrap();
        let Value::Object(getter) = context.eval("(function(){return 1})").unwrap() else {
            panic!("getter")
        };
        let key = runtime.intern_property_key("published-symbol").unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        state
            .define_raw_property(
                owner.object_id(),
                key.atom(),
                &PropertyDescriptor {
                    get: Some(Some(RawValue::Object(getter.object_id()))),
                    configurable: Some(true),
                    ..PropertyDescriptor::new()
                },
            )
            .unwrap();
        if dictionary {
            state
                .ensure_dictionary_layout_with_poison(&runtime.0.poisoned, owner.object_id())
                .unwrap();
        }
        let atom = state.atoms.new_symbol(Some("published-edge")).unwrap();
        let index = state.atoms.unbrand(atom).unwrap();
        let before = state.heap.object_strong_count(owner.object_id()).unwrap();
        state.heap.retain_object(owner.object_id()).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(getter.object_id()), 0);
        {
            let mut guard = OwnedValueGuard::new(
                &mut state,
                &runtime.0.poisoned,
                JsValue::Object(owner.object_id()),
            );
            let (state, _) = guard.parts();
            assert!(
                state
                    .define_raw_property_with_poison(
                        &runtime.0.poisoned,
                        owner.object_id(),
                        key.atom(),
                        &PropertyDescriptor {
                            value: Some(RawValue::Symbol(index)),
                            writable: Some(true),
                            ..PropertyDescriptor::new()
                        },
                    )
                    .is_err()
            );
            assert!(runtime.is_poisoned());
            assert_slot(
                slot(state, owner.object_id(), key.atom()),
                Some(PropertySlot::Data(RawValue::Symbol(index))),
            );
            assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(2));
        }
        assert_eq!(
            state.heap.object_strong_count(owner.object_id()),
            Ok(before + 1),
            "owner guard must abandon its edge after published cleanup failure"
        );
        assert_eq!(
            state.atoms.resolve(atom).unwrap().ref_count,
            Some(2),
            "published slot keeps its atom edge"
        );
    }
}

#[test]
fn heap_layout_status_distinguishes_prepare_failure_from_published_cleanup() {
    for published in [false, true] {
        let mut heap = Heap::new();
        let atom = AtomIdx::from_raw(8);
        let flags = PropertyFlags::data(true, true, true);
        let shape = heap
            .allocate_shape(Shape::new(None, vec![ShapeEntry { atom, flags }]).unwrap())
            .unwrap();
        let empty = heap
            .allocate_shape(Shape::new(None, vec![]).unwrap())
            .unwrap();
        let child = heap
            .allocate_object(ObjectData::ordinary(empty, vec![]))
            .unwrap();
        let object = heap
            .allocate_object(ObjectData::ordinary(
                shape,
                vec![PropertySlot::Data(RawValue::Object(child))],
            ))
            .unwrap();
        let replacement = if published {
            heap.set_strong_count_for_test(RawId::Object(child), 0);
            PropertySlot::Data(RawValue::Int(9))
        } else {
            PropertySlot::accessor(None, None)
        };
        let error = heap
            .replace_object_layout_with_status(
                object,
                shape,
                Slots::from_vec(vec![replacement.clone()]),
            )
            .unwrap_err();
        assert_eq!(error.published, published);
        assert_slot(
            Some(heap.object(object).unwrap().slots[0].clone()),
            Some(if published {
                replacement
            } else {
                PropertySlot::Data(RawValue::Object(child))
            }),
        );
    }
}

#[test]
fn heap_dictionary_status_comes_from_actual_delete_or_flags_publication() {
    for delete in [false, true] {
        for published in [false, true] {
            let mut heap = Heap::new();
            let atom = Atom::from_raw(8);
            let flags = PropertyFlags::data(true, true, true);
            let shape = heap
                .allocate_shape(
                    Shape::new(
                        None,
                        vec![ShapeEntry {
                            atom: AtomIdx::from_raw(atom.raw()),
                            flags,
                        }],
                    )
                    .unwrap(),
                )
                .unwrap();
            let empty = heap
                .allocate_shape(Shape::new(None, vec![]).unwrap())
                .unwrap();
            let child = heap
                .allocate_object(ObjectData::ordinary(empty, vec![]))
                .unwrap();
            let object = heap
                .allocate_object(ObjectData::ordinary(
                    shape,
                    vec![PropertySlot::Data(RawValue::Object(child))],
                ))
                .unwrap();
            let cleanup = heap.release_shape(shape).unwrap();
            assert_eq!(cleanup, Default::default());
            heap.enable_object_dictionary(object).unwrap();
            if published {
                heap.set_strong_count_for_test(RawId::Object(child), 0);
            }
            let error = if delete {
                heap.delete_dictionary_property_with_status(
                    object,
                    if published { atom } else { Atom::from_raw(9) },
                )
            } else {
                heap.replace_dictionary_property_with_status(
                    object,
                    if published { 0 } else { 1 },
                    PropertyFlags::data(false, false, true),
                    PropertySlot::Data(RawValue::Int(9)),
                )
            }
            .unwrap_err();
            assert_eq!(error.published, published);
            let object = heap.object(object).unwrap();
            let shape = heap.shape(object.shape).unwrap();
            if delete && published {
                assert!(shape.entries().is_empty() && object.slots.is_empty());
            } else if published {
                assert_eq!(
                    shape.entries()[0].flags,
                    PropertyFlags::data(false, false, true)
                );
                assert_slot(
                    Some(object.slots[0].clone()),
                    Some(PropertySlot::Data(RawValue::Int(9))),
                );
            } else {
                assert_eq!(shape.entries()[0].flags, flags);
                assert_slot(
                    Some(object.slots[0].clone()),
                    Some(PropertySlot::Data(RawValue::Object(child))),
                );
            }
        }
    }
}
