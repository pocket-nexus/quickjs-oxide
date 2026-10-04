use super::*;

#[test]
fn definition_array_dense_index_retain_failure_is_recoverable_before_commit() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let array = object(&mut context, "[1]");
    let child = runtime.new_object(None).unwrap();
    let key = runtime.property_key_for_index(0).unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), u32::MAX);
    let result = define_raw(
        &runtime,
        &array,
        &key,
        &PropertyDescriptor {
            value: Some(RawValue::Object(child.object_id())),
            ..PropertyDescriptor::new()
        },
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    let state = runtime.0.state.borrow();
    assert!(matches!(
        dense_value(&state, array.object_id(), 0),
        Some(RawValue::Int(1))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_array_dense_replacement_quarantines_before_suffix_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let array = object(&mut context, "[1,2]");
    let key = runtime.property_key_for_index(0).unwrap();
    let (first, later) = invalid_zero_queue(&runtime);
    assert_eq!(
        define_raw(
            &runtime,
            &array,
            &key,
            &PropertyDescriptor {
                value: Some(RawValue::Int(9)),
                ..PropertyDescriptor::new()
            }
        ),
        Err(queue_error())
    );
    assert_stopped_zero_suffix(&runtime, first, later);
    assert!(matches!(
        dense_value(&runtime.0.state.borrow(), array.object_id(), 0),
        Some(RawValue::Int(9))
    ));
    assert_eq!(runtime.array_length_state(&array), Ok((2, true)));
}

#[test]
fn definition_array_materialization_failure_does_not_publish_requested_accessor() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let array = object(&mut context, "[1,2]");
    let key = runtime.property_key_for_index(0).unwrap();
    let (first, later) = invalid_zero_queue(&runtime);
    assert_eq!(
        define_raw(
            &runtime,
            &array,
            &key,
            &PropertyDescriptor {
                get: Some(None),
                ..PropertyDescriptor::new()
            }
        ),
        Err(queue_error())
    );
    assert_stopped_zero_suffix(&runtime, first, later);
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.array_dense_len(array.object_id()), Ok(None));
    assert!(matches!(
        slot(&state, array.object_id(), &key),
        PropertySlot::Data(RawValue::Int(1))
    ));
}

#[test]
fn definition_array_recovery_failure_stops_after_owner_move_and_before_retiring_shape() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let array = object(&mut context, "(()=>{const a=[]; a[1]=2; return a;})()");
    assert_eq!(runtime.array_fast_len(&array), Ok(None));
    let key = runtime.property_key_for_index(0).unwrap();
    let (first, later) = invalid_zero_queue(&runtime);
    assert_eq!(
        define_raw(
            &runtime,
            &array,
            &key,
            &PropertyDescriptor {
                value: Some(RawValue::Int(1)),
                writable: Some(true),
                enumerable: Some(true),
                configurable: Some(true),
                ..PropertyDescriptor::new()
            }
        ),
        Err(queue_error())
    );
    assert_stopped_zero_suffix(&runtime, first, later);
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.array_dense_len(array.object_id()), Ok(Some(2)));
    assert!(matches!(
        dense_value(&state, array.object_id(), 0),
        Some(RawValue::Int(1))
    ));
    assert!(matches!(
        dense_value(&state, array.object_id(), 1),
        Some(RawValue::Int(2))
    ));
}

#[test]
fn definition_array_dense_truncation_failure_precedes_final_read_only_publication() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let array = object(&mut context, "[1,2,3]");
    let key = runtime.intern_property_key("length").unwrap();
    let (first, later) = invalid_zero_queue(&runtime);
    let result = runtime.apply_array_length_descriptor(
        &array,
        &key,
        &OrdinaryPropertyDescriptor {
            writable: DescriptorField::Present(false),
            ..OrdinaryPropertyDescriptor::new()
        },
        1,
    );
    assert!(matches!(result, Err(ref error) if *error == queue_error()));
    assert_stopped_zero_suffix(&runtime, first, later);
    assert_eq!(runtime.array_length_state(&array), Ok((1, true)));
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .array_dense_len(array.object_id()),
        Ok(Some(1))
    );
}

#[test]
fn definition_array_sparse_truncation_failure_precedes_blocker_rollback() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let array = object(
        &mut context,
        "(()=>{const a=[]; a[20]=20; Object.defineProperty(a,'10',{value:10,configurable:false}); return a;})()",
    );
    let key = runtime.intern_property_key("length").unwrap();
    let twenty = runtime.property_key_for_index(20).unwrap();
    let (first, later) = invalid_zero_queue(&runtime);
    let result = runtime.apply_array_length_descriptor(
        &array,
        &key,
        &OrdinaryPropertyDescriptor {
            writable: DescriptorField::Present(false),
            ..OrdinaryPropertyDescriptor::new()
        },
        5,
    );
    assert!(matches!(result, Err(ref error) if *error == queue_error()));
    assert_stopped_zero_suffix(&runtime, first, later);
    // Length=5 was published first. The canonical sparse removal committed,
    // but the later restore-to-11/read-only transition must not follow failure.
    assert_eq!(runtime.array_length_state(&array), Ok((5, true)));
    let state = runtime.0.state.borrow();
    let data = state.heap.object(array.object_id()).unwrap();
    assert!(
        state
            .heap
            .shape(data.shape)
            .unwrap()
            .find(AtomIdx::from_raw(twenty.atom().raw()))
            .is_none()
    );
}

#[test]
fn definition_array_owned_read_only_length_precedes_mixed_descriptor() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let array = object(
        &mut context,
        "Object.defineProperty([], 'length', {writable:false})",
    );
    let key = runtime.property_key_for_index(0).unwrap();
    let mut descriptor = OwnedPropertyDescriptor::new(&runtime);
    descriptor.value = DescriptorField::Present(JsValue::Int(1));
    descriptor.get = DescriptorField::Present(AccessorValue::Undefined);
    assert_eq!(
        runtime.define_owned_property_after_conversion_selection(&array, &key, &descriptor),
        Ok(false)
    );
    let public = OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(Value::Int(1)),
        get: DescriptorField::Present(AccessorValue::Undefined),
        ..OrdinaryPropertyDescriptor::new()
    };
    assert!(matches!(
        runtime.define_own_property(&array, &key, &public),
        Err(RuntimeError::Property(
            crate::engine::object::property::PropertyDefinitionError::InvalidDescriptor
        ))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_array_length_conversion_reloads_flags_and_preserves_two_number_calls() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval("(()=>{let calls=0; const a=[1,2,3]; const n={valueOf(){calls++; if(calls===2)Object.defineProperty(a,'length',{writable:false}); return 1;}}; const ok=Reflect.defineProperty(a,'length',{value:n}); return !ok&&calls===2&&a.length===3;})()").unwrap(), Value::Bool(true));
}
