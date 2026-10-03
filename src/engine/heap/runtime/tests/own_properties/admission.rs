use super::*;

#[test]
fn own_foreign_key_is_rejected_before_internal_pending_cleanup() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let key = foreign.intern_property_key("foreign").unwrap();
    let later = pending_invalid(&runtime);
    assert!(matches!(
        runtime.get_own_property_owned(&object, &key),
        Err(RuntimeError::WrongRuntime("property key"))
    ));
    assert_eq!(runtime.0.deferred_references.borrow().len(), 2);
    assert!(!runtime.is_poisoned());
    assert!(matches!(
        runtime.drain_deferred_references(),
        Err(RuntimeError::Heap(HeapError::Stale { .. }))
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(later),
        Ok(1)
    );
}

#[test]
fn own_domains_precede_internal_admission_but_public_admission_precedes_domains() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let object = foreign.new_object(None).unwrap();
    let key = runtime.intern_property_key("missing").unwrap();
    let later = pending_invalid(&runtime);
    assert!(matches!(
        runtime.get_own_property_owned(&object, &key),
        Err(RuntimeError::WrongRuntime("object"))
    ));
    assert_eq!(runtime.0.deferred_references.borrow().len(), 2);
    assert!(!runtime.is_poisoned());
    assert!(matches!(
        runtime.get_own_property(&object, &key),
        Err(RuntimeError::Heap(HeapError::Stale { .. }))
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(later),
        Ok(1)
    );
}

#[test]
fn own_ready_scalar_uses_new_internal_operation_admission_and_fifo_first_error() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("scalar").unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::Data(RawValue::Int(3)),
    );
    let later = pending_invalid(&runtime);
    assert!(matches!(
        runtime.get_own_property_owned(&object, &key),
        Err(RuntimeError::Heap(HeapError::Stale { .. }))
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(later),
        Ok(1)
    );
}

#[test]
fn own_scalar_does_not_infer_poison_from_untraversed_heap_zero_queue() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("scalar").unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::Data(RawValue::Int(3)),
    );
    let (first, _) = invalid_zero_queue(&runtime);
    let owned = runtime
        .get_own_property_owned(&object, &key)
        .unwrap()
        .unwrap();
    assert!(matches!(data_value(&owned), RawValue::Int(3)));
    assert!(!runtime.is_poisoned());
    drop(owned);
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(first), 0);
    let cleanup = state.heap.drain_zero_queue().unwrap();
    state.apply_cleanup(cleanup).unwrap();
}
