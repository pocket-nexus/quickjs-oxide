use super::*;

fn mapped(
    runtime: &Runtime,
    context: &mut crate::engine::api::context::Context,
    source: &str,
) -> (ObjectRef, PropertyKey, VarRefRoot) {
    let arguments = object(context, source);
    let key = runtime.property_key_for_index(0).unwrap();
    let cell = runtime.own_var_ref_root(&arguments, &key).unwrap().unwrap();
    (arguments, key, cell)
}

#[test]
fn definition_owned_mapped_accessor_borrows_current_at_max_without_unneeded_promotion() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) = mapped(
        &runtime,
        &mut context,
        "(function(a){return arguments})({tag:1})",
    );
    let RawValue::Object(child) = runtime
        .0
        .state
        .borrow()
        .heap
        .var_ref(cell.id())
        .unwrap()
        .value
    else {
        panic!("cell object")
    };
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child), u32::MAX);
    let mut descriptor = OwnedPropertyDescriptor::new(&runtime);
    descriptor.get = DescriptorField::Present(AccessorValue::Undefined);
    let result =
        runtime.define_owned_property_after_conversion_selection(&arguments, &key, &descriptor);
    let count = runtime.0.state.borrow().heap.object_strong_count(child);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child), 1);
    assert_eq!(result, Ok(true));
    assert_eq!(count, Ok(u32::MAX));
    assert!(matches!(
        slot(&runtime.0.state.borrow(), arguments.object_id(), &key),
        PropertySlot::Accessor { get, set } if get.option().is_none() && set.option().is_none()
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_public_mapped_current_promotion_still_fails_checked_at_max() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) = mapped(
        &runtime,
        &mut context,
        "(function(a){return arguments})({tag:1})",
    );
    let RawValue::Object(child) = runtime
        .0
        .state
        .borrow()
        .heap
        .var_ref(cell.id())
        .unwrap()
        .value
    else {
        panic!("cell object")
    };
    let before_cell_count = runtime
        .0
        .state
        .borrow()
        .heap
        .var_ref_strong_count(cell.id())
        .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child), u32::MAX);
    let result = runtime.define_own_property(
        &arguments,
        &key,
        &OrdinaryPropertyDescriptor {
            get: DescriptorField::Present(AccessorValue::Undefined),
            ..OrdinaryPropertyDescriptor::new()
        },
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .var_ref_strong_count(cell.id()),
        Ok(before_cell_count)
    );
    assert!(
        matches!(slot(&runtime.0.state.borrow(), arguments.object_id(), &key), PropertySlot::VarRef(id) if id == cell.id())
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_public_mapped_completion_requests_independent_checked_object_role() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) = mapped(
        &runtime,
        &mut context,
        "(function(a){return arguments})({tag:1})",
    );
    let RawValue::Object(child) = runtime
        .0
        .state
        .borrow()
        .heap
        .var_ref(cell.id())
        .unwrap()
        .value
    else {
        panic!("cell object")
    };
    // One available count admits current's public promotion; the independent
    // completed public descriptor promotion must fail before the cell write.
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child), u32::MAX - 1);
    let result = runtime.define_own_property(
        &arguments,
        &key,
        &OrdinaryPropertyDescriptor {
            enumerable: DescriptorField::Present(false),
            ..OrdinaryPropertyDescriptor::new()
        },
    );
    let count = runtime.0.state.borrow().heap.object_strong_count(child);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    // The accepted current promotion reached the existing immortal MAX
    // sentinel; retirement preserves it, while completion retain rejected.
    assert_eq!(count, Ok(u32::MAX));
    assert!(
        matches!(slot(&runtime.0.state.borrow(), arguments.object_id(), &key), PropertySlot::VarRef(id) if id == cell.id())
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_public_mapped_read_only_string_recreates_distinct_cell_and_slot_producers() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) = mapped(
        &runtime,
        &mut context,
        "(function(a){return arguments})('old')",
    );
    assert_eq!(
        runtime.define_own_property(
            &arguments,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::String(JsString::from_static(
                    "new payload"
                ))),
                writable: DescriptorField::Present(false),
                ..OrdinaryPropertyDescriptor::new()
            }
        ),
        Ok(true)
    );
    let state = runtime.0.state.borrow();
    let RawValue::String(cell_value) = state.heap.var_ref(cell.id()).unwrap().value else {
        panic!("cell String")
    };
    let PropertySlot::Data(RawValue::String(slot_value)) =
        slot(&state, arguments.object_id(), &key)
    else {
        panic!("detached String slot")
    };
    assert_ne!(cell_value, slot_value);
    assert_eq!(state.heap.string(cell_value), state.heap.string(slot_value));
    assert_eq!(state.heap.strong_count(RawId::String(cell_value)), Ok(1));
    assert_eq!(state.heap.strong_count(RawId::String(slot_value)), Ok(1));
}

#[test]
fn definition_owned_mapped_read_only_string_keeps_same_checked_arena_identity() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) = mapped(
        &runtime,
        &mut context,
        "(function(a){return arguments})('old')",
    );
    let value = runtime
        .into_jsvalue(Value::String(JsString::from_static("new payload")))
        .unwrap();
    let JsValue::String(value_id) = value else {
        panic!("owned String")
    };
    let mut descriptor = OwnedPropertyDescriptor::new(&runtime);
    descriptor.value = DescriptorField::Present(value);
    descriptor.writable = DescriptorField::Present(false);
    assert_eq!(
        runtime.define_owned_property_after_conversion_selection(&arguments, &key, &descriptor),
        Ok(true)
    );
    let state = runtime.0.state.borrow();
    assert!(
        matches!(state.heap.var_ref(cell.id()).unwrap().value, RawValue::String(id) if id == value_id)
    );
    assert!(
        matches!(slot(&state, arguments.object_id(), &key), PropertySlot::Data(RawValue::String(id)) if id == value_id)
    );
    assert_eq!(state.heap.strong_count(RawId::String(value_id)), Ok(3));
}

#[test]
fn definition_public_partial_accessor_cleanup_error_wins_before_mapped_slot_publication() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) =
        mapped(&runtime, &mut context, "(function(a){return arguments})(1)");
    let getter = object(&mut context, "(function(){})");
    let setter = object(&mut context, "(function(v){})");
    let descriptor = OrdinaryPropertyDescriptor {
        get: DescriptorField::Present(AccessorValue::Callable(
            runtime.as_callable(&getter).unwrap().unwrap(),
        )),
        set: DescriptorField::Present(AccessorValue::Callable(
            runtime.as_callable(&setter).unwrap().unwrap(),
        )),
        ..OrdinaryPropertyDescriptor::new()
    };
    let cell_count = runtime
        .0
        .state
        .borrow()
        .heap
        .var_ref_strong_count(cell.id())
        .unwrap();
    let getter_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(getter.object_id())
        .unwrap();
    let setter_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(setter.object_id())
        .unwrap();
    let (first, later) = invalid_zero_queue(&runtime);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
    assert_eq!(
        runtime.define_own_property(&arguments, &key, &descriptor),
        Err(queue_error())
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), setter_count);
    assert_stopped_zero_suffix(&runtime, first, later);
    let state = runtime.0.state.borrow();
    assert_eq!(
        state.heap.object_strong_count(getter.object_id()),
        Ok(getter_count)
    );
    // The checked temporary cell owner remains quarantined after the first
    // failed cleanup, and the accessor publication never follows it.
    assert_eq!(
        state.heap.var_ref_strong_count(cell.id()),
        Ok(cell_count + 1)
    );
    assert!(
        matches!(slot(&state, arguments.object_id(), &key), PropertySlot::VarRef(id) if id == cell.id())
    );
}

#[test]
fn definition_mapped_cell_write_cleanup_failure_precedes_detachment() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) = mapped(
        &runtime,
        &mut context,
        "(function(a){return arguments})({tag:1})",
    );
    let (first, later) = invalid_zero_queue(&runtime);
    assert_eq!(
        define_raw(
            &runtime,
            &arguments,
            &key,
            &PropertyDescriptor {
                value: Some(RawValue::Int(9)),
                writable: Some(false),
                ..PropertyDescriptor::new()
            }
        ),
        Err(queue_error())
    );
    assert_stopped_zero_suffix(&runtime, first, later);
    let state = runtime.0.state.borrow();
    assert!(matches!(
        state.heap.var_ref(cell.id()).unwrap().value,
        RawValue::Int(9)
    ));
    assert!(
        matches!(slot(&state, arguments.object_id(), &key), PropertySlot::VarRef(id) if id == cell.id())
    );
}

#[test]
fn definition_unmapped_arguments_invalidates_fast_prefix_before_rejection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let arguments = object(
        &mut context,
        "(function(){'use strict';return arguments})(1)",
    );
    let key = runtime.property_key_for_index(0).unwrap();
    assert_eq!(runtime.arguments_fast_len(&arguments), Ok(Some(1)));
    runtime
        .define_own_property(
            &arguments,
            &key,
            &OrdinaryPropertyDescriptor {
                configurable: DescriptorField::Present(false),
                ..OrdinaryPropertyDescriptor::new()
            },
        )
        .unwrap();
    assert_eq!(runtime.arguments_fast_len(&arguments), Ok(None));
    assert_eq!(
        define_raw(
            &runtime,
            &arguments,
            &key,
            &PropertyDescriptor {
                configurable: Some(true),
                ..PropertyDescriptor::new()
            }
        ),
        Ok(false)
    );
}

#[test]
fn definition_public_mapped_read_only_bigint_recreates_distinct_heap_producers() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let (arguments, key, cell) = mapped(
        &runtime,
        &mut context,
        "(function(a){return arguments})(18446744073709551615n)",
    );
    assert_eq!(
        runtime.define_own_property(
            &arguments,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::BigInt(JsBigInt::from(u64::MAX))),
                writable: DescriptorField::Present(false),
                ..OrdinaryPropertyDescriptor::new()
            }
        ),
        Ok(true)
    );
    let state = runtime.0.state.borrow();
    let RawValue::BigInt(cell_value) = state.heap.var_ref(cell.id()).unwrap().value else {
        panic!("cell heap BigInt")
    };
    let PropertySlot::Data(RawValue::BigInt(slot_value)) =
        slot(&state, arguments.object_id(), &key)
    else {
        panic!("detached heap BigInt")
    };
    assert_ne!(cell_value, slot_value);
    assert_eq!(state.heap.bigint(cell_value), state.heap.bigint(slot_value));
    assert_eq!(state.heap.strong_count(RawId::BigInt(cell_value)), Ok(1));
    assert_eq!(state.heap.strong_count(RawId::BigInt(slot_value)), Ok(1));
}
