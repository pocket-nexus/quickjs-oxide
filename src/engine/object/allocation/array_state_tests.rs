//! Array construction adopts owners and keeps each failure's suffix accountable.
use super::*;
use crate::engine::heap::{HeapError, RawId};
use crate::engine::value::bigint::JsBigInt;

fn missing_realm(runtime: &Runtime) -> ContextId {
    let context = runtime.new_context().unwrap();
    let realm = context.realm;
    drop(context);
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().heap.context(realm).is_err());
    realm
}

fn stale_object(runtime: &Runtime) -> ObjectId {
    let object = runtime.new_object(None).unwrap().into_handle();
    runtime.release_jsvalue(JsValue::Object(object)).unwrap();
    object
}

fn diverge_length(state: &mut RuntimeState, array: ObjectId) {
    let cleanup = state
        .heap
        .replace_object_slot(array, 0, PropertySlot::Data(RawValue::Int(2)))
        .unwrap();
    state.apply_cleanup(cleanup).unwrap();
}

#[test]
fn state_array_adopts_aliases_leaf_and_symbol_owners_in_order_with_length_slot_zero() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap().into_handle();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let string = state
        .heap
        .allocate_string(JsString::from_static("array"))
        .unwrap();
    let bigint = state.heap.allocate_bigint(JsBigInt::one()).unwrap();
    let atom = state.atoms.new_symbol(Some("array")).unwrap();
    let symbol = state.atoms.unbrand(atom).unwrap();
    state.heap.retain_object(object).unwrap();
    let array = state
        .new_array_from_values_jsvalue(
            &runtime.0.poisoned,
            context.realm,
            vec![
                JsValue::Object(object),
                JsValue::String(string),
                JsValue::Symbol(symbol),
                JsValue::Object(object),
                JsValue::BigInt(bigint),
                JsValue::Int(42),
            ],
        )
        .unwrap();
    assert_eq!(state.heap.object_strong_count(array), Ok(1));
    assert_eq!(state.heap.object_strong_count(object), Ok(2));
    assert_eq!(state.heap.strong_count(RawId::String(string)), Ok(1));
    assert_eq!(state.heap.strong_count(RawId::BigInt(bigint)), Ok(1));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
    let data = state.heap.object(array).unwrap();
    let shape = state.heap.shape(data.shape).unwrap();
    assert_eq!(
        shape.prototype(),
        Some(state.heap.context(context.realm).unwrap().array_prototype)
    );
    assert_eq!(
        shape.entries()[0].atom.raw(),
        state
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Length)
            .raw()
    );
    assert_eq!(
        shape.entries()[0].flags,
        PropertyFlags::data(true, false, false)
    );
    assert!(matches!(
        data.slots.first(),
        Some(PropertySlot::Data(RawValue::Int(6)))
    ));
    // These comparison views borrow storage identities; they own no edge.
    for (index, expected) in [
        JsValue::Object(object),
        JsValue::String(string),
        JsValue::Symbol(symbol),
        JsValue::Object(object),
        JsValue::BigInt(bigint),
        JsValue::Int(42),
    ]
    .into_iter()
    .enumerate()
    {
        let stored = data
            .dense_array_value(index as u32)
            .and_then(|raw| JsValue::from_raw(raw.clone()));
        assert_eq!(stored, Some(expected));
    }
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    assert!(!runtime.0.deferred_references.has_pending());
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(array))
        .unwrap();
    assert!(state.heap.object(object).is_err());
    assert!(state.heap.string(string).is_err());
    assert!(state.heap.bigint(bigint).is_err());
    assert!(state.atoms.resolve(atom).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn state_array_append_adopts_saturated_element_owners_without_checked_retain() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let atom = state.atoms.new_symbol(Some("saturated")).unwrap();
    let symbol = state.atoms.unbrand(atom).unwrap();
    let array = state.new_array(&runtime.0.poisoned, context.realm).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object), u32::MAX);
    state.atoms.set_ref_count_for_test(symbol, u32::MAX);
    let object_result =
        state.append_fresh_array_value_jsvalue(&runtime.0.poisoned, array, JsValue::Object(object));
    let symbol_result =
        state.append_fresh_array_value_jsvalue(&runtime.0.poisoned, array, JsValue::Symbol(symbol));
    let object_count = state.heap.object_strong_count(object);
    let symbol_count = state.atoms.resolve(atom).unwrap().ref_count;
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object), 1);
    state.atoms.set_ref_count_for_test(symbol, 1);
    assert_eq!(object_result, Ok(()));
    assert_eq!(symbol_result, Ok(()));
    assert_eq!(object_count, Ok(u32::MAX));
    assert_eq!(symbol_count, Some(u32::MAX));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(array))
        .unwrap();
    assert!(state.heap.object(object).is_err());
    assert!(state.atoms.resolve(atom).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn state_array_cached_layout_keeps_checked_prototype_retain_and_releases_all_inputs() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let warm = runtime.new_array(context.realm).unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let last = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let shape = state.heap.object(warm.object_id()).unwrap().shape;
    assert!(state.shape_is_canonical(shape));
    let prototype = state.heap.context(context.realm).unwrap().array_prototype;
    let count = state.heap.object_strong_count(prototype).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
    let result = state.new_array_from_values_jsvalue(
        &runtime.0.poisoned,
        context.realm,
        vec![JsValue::Object(first), JsValue::Object(last)],
    );
    let blocked_count = state.heap.object_strong_count(prototype);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), count);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(blocked_count, Ok(u32::MAX));
    assert!(state.heap.object(first).is_err());
    assert!(state.heap.object(last).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn state_array_rejected_append_retires_array_and_prefix_before_guarded_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prefix = state
        .heap
        .allocate_string(JsString::from_static("prefix"))
        .unwrap();
    let rejected = state
        .heap
        .allocate_string(JsString::from_static("rejected"))
        .unwrap();
    let array = state.new_array(&runtime.0.poisoned, context.realm).unwrap();
    state
        .append_fresh_array_value_jsvalue(&runtime.0.poisoned, array, JsValue::String(prefix))
        .unwrap();
    diverge_length(&mut state, array);
    {
        let mut values = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::String(rejected), JsValue::Object(suffix)],
        );
        let (state, values) = values.parts();
        let error = state
            .append_fresh_array_values_owned(&runtime.0.poisoned, array, values)
            .unwrap_err();
        assert!(matches!(
            error,
            RuntimeError::Heap(HeapError::Invariant(
                "fresh Array length diverged from its dense count"
            ))
        ));
        assert!(state.heap.object(array).is_err());
        assert!(state.heap.string(prefix).is_err());
        assert!(state.heap.string(rejected).is_err());
        assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
        assert_eq!(values, &[JsValue::Undefined, JsValue::Object(suffix)]);
    }
    assert!(state.heap.object(suffix).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn state_array_prefix_cleanup_failure_precedes_append_error_and_stops_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let rejected = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let stale = stale_object(&runtime);
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let array = state.new_array(&runtime.0.poisoned, context.realm).unwrap();
    state
        .append_fresh_array_value_jsvalue(&runtime.0.poisoned, array, JsValue::Object(stale))
        .unwrap();
    diverge_length(&mut state, array);
    {
        let mut values = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(rejected), JsValue::Object(suffix)],
        );
        let (state, values) = values.parts();
        assert_eq!(
            state.append_fresh_array_values_owned(&runtime.0.poisoned, array, values),
            Err(RuntimeError::Heap(HeapError::Stale {
                index: stale.debug_index(),
                generation: stale.debug_generation()
            }))
        );
        assert!(state.heap.object(rejected).is_err());
        assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
        assert!(runtime.is_poisoned());
    }
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
}

#[test]
fn state_array_failed_allocation_releases_suffix_in_order_and_stops_at_first_cleanup_error() {
    let runtime = Runtime::new();
    let realm = missing_realm(&runtime);
    let first = runtime.new_object(None).unwrap().into_handle();
    let last = runtime.new_object(None).unwrap().into_handle();
    let stale = stale_object(&runtime);
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    assert_eq!(
        state.new_array_from_values_jsvalue(
            &runtime.0.poisoned,
            realm,
            vec![
                JsValue::Object(first),
                JsValue::Object(stale),
                JsValue::Object(last)
            ]
        ),
        Err(RuntimeError::Heap(HeapError::Stale {
            index: stale.debug_index(),
            generation: stale.debug_generation()
        }))
    );
    assert!(state.heap.object(first).is_err());
    assert_eq!(state.heap.object_strong_count(last), Ok(1));
    assert!(runtime.is_poisoned());
}

#[test]
fn runtime_array_failed_admission_keeps_outer_suffix_and_stops_after_poison() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let last = runtime.new_object(None).unwrap().into_handle();
    let stale = stale_object(&runtime);
    runtime
        .0
        .deferred_references
        .push_back(crate::engine::heap::runtime::DeferredRefOp::Object(stale));
    assert!(
        runtime
            .new_array_from_values_jsvalue(
                context.realm,
                vec![JsValue::Object(first), JsValue::Object(last)]
            )
            .is_err()
    );
    let state = runtime.0.state.borrow();
    // Admission quarantines state before construction or suffix traversal.
    assert_eq!(state.heap.object_strong_count(first), Ok(1));
    assert_eq!(state.heap.object_strong_count(last), Ok(1));
    assert!(runtime.is_poisoned());
}
