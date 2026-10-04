use super::*;

fn pending_invalid(runtime: &Runtime) -> ObjectId {
    let first = runtime.new_object(None).unwrap().into_handle();
    runtime.release_jsvalue(JsValue::Object(first)).unwrap();
    let later = runtime.new_object(None).unwrap().into_handle();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(first));
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(later));
    later
}

#[test]
fn definition_internal_domains_precede_admission_public_admission_precedes_domains() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let receiver = foreign.new_object(None).unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    let descriptor = OwnedPropertyDescriptor::new(&runtime);
    let later = pending_invalid(&runtime);
    assert_eq!(
        runtime.define_owned_property_after_conversion_selection(&receiver, &key, &descriptor),
        Err(RuntimeError::WrongRuntime("object"))
    );
    assert!(!runtime.is_poisoned());
    assert_eq!(runtime.0.deferred_references.borrow().len(), 2);
    assert!(matches!(
        runtime.define_own_property(&receiver, &key, &OrdinaryPropertyDescriptor::new()),
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
fn definition_foreign_public_descriptor_precedes_internal_pending_cleanup() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    let descriptor = data_descriptor(
        Value::Object(foreign.new_object(None).unwrap()),
        true,
        true,
        true,
    );
    let _later = pending_invalid(&runtime);
    assert_eq!(
        runtime.define_ordinary_own_property(&receiver, &key, &descriptor),
        Err(RuntimeError::WrongRuntime("descriptor value"))
    );
    assert!(!runtime.is_poisoned());
    assert_eq!(runtime.0.deferred_references.borrow().len(), 2);
    assert!(runtime.drain_deferred_references().is_err());
    assert!(runtime.is_poisoned());
}

#[test]
fn definition_internal_scalar_admission_stops_at_fifo_error_before_publication() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::Data(RawValue::Int(1)),
    );
    let mut descriptor = OwnedPropertyDescriptor::new(&runtime);
    descriptor.value = DescriptorField::Present(JsValue::Int(2));
    let later = pending_invalid(&runtime);
    assert!(matches!(
        runtime.define_owned_property_after_conversion_selection(&receiver, &key, &descriptor),
        Err(RuntimeError::Heap(HeapError::Stale { .. }))
    ));
    assert!(matches!(
        slot(&runtime.0.state.borrow(), receiver.object_id(), &key),
        PropertySlot::Data(RawValue::Int(1))
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(runtime.0.deferred_references.borrow().len(), 1);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(later),
        Ok(1)
    );
}

#[test]
fn definition_scalar_commit_does_not_infer_poison_from_untraversed_zero_queue() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::Data(RawValue::Int(1)),
    );
    let (first, later) = invalid_zero_queue(&runtime);
    assert_eq!(
        define_raw(
            &runtime,
            &receiver,
            &key,
            &PropertyDescriptor {
                value: Some(RawValue::Int(2)),
                ..PropertyDescriptor::new()
            }
        ),
        Ok(true)
    );
    assert!(!runtime.is_poisoned());
    let mut state = runtime.0.state.borrow_mut();
    assert_eq!(state.heap.zero_queue.front(), Some(&RawId::Object(first)));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    state
        .heap
        .set_strong_count_for_test(RawId::Object(first), 0);
    let cleanup = state.heap.drain_zero_queue().unwrap();
    state.apply_cleanup(cleanup).unwrap();
}
