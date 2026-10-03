use super::*;

fn object(context: &mut crate::engine::api::context::Context, source: &str) -> ObjectRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("own-property fixture did not return an object")
    };
    object
}

#[test]
fn own_string_code_units_dense_holes_and_stored_flags_share_selection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let wrapper = object(&mut context, r#"new String('\ud800x')"#);
    let zero = runtime.intern_property_key("0").unwrap();
    let public = runtime.get_own_property(&wrapper, &zero).unwrap().unwrap();
    let CompleteOrdinaryPropertyDescriptor::Data {
        value: Value::String(unit),
        writable,
        enumerable,
        configurable,
    } = public
    else {
        panic!("String index descriptor")
    };
    assert_eq!(unit.utf16_units().collect::<Vec<_>>(), vec![0xd800]);
    assert!(!writable && enumerable && !configurable);
    let owned = runtime
        .get_own_property_owned(&wrapper, &zero)
        .unwrap()
        .unwrap();
    let RawValue::String(id) = data_value(&owned) else {
        panic!("owned String index")
    };
    let id = *id;
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .strong_count(RawId::String(id)),
        Ok(1)
    );
    drop(owned);
    assert!(runtime.0.state.borrow().heap.string(id).is_err());

    let array = object(&mut context, "[7,,9]");
    let one = runtime.intern_property_key("1").unwrap();
    assert!(
        runtime
            .get_own_property_owned(&array, &one)
            .unwrap()
            .is_none()
    );
    runtime
        .define_own_property(
            &array,
            &one,
            &data_descriptor(Value::Int(8), false, false, true),
        )
        .unwrap();
    let own = runtime
        .get_own_property_owned(&array, &one)
        .unwrap()
        .unwrap();
    assert!(matches!(
        own.record(),
        CompletePropertyDescriptor::Data {
            value: RawValue::Int(8),
            writable: false,
            enumerable: false,
            configurable: true,
        }
    ));
    let dense = runtime
        .get_own_property_owned(&array, &zero)
        .unwrap()
        .unwrap();
    assert!(matches!(
        dense.record(),
        CompletePropertyDescriptor::Data {
            value: RawValue::Int(7),
            writable: true,
            enumerable: true,
            configurable: true,
        }
    ));
}

#[test]
fn own_virtual_string_cleanup_failure_does_not_expose_descriptor_or_traverse_suffix() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let wrapper = object(&mut context, "new String('x')");
    let zero = runtime.intern_property_key("0").unwrap();
    let (first, later) = invalid_zero_queue(&runtime);
    let result = runtime.get_own_property_owned(&wrapper, &zero);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Invariant(
            "finalization count disagrees with its queue/cycle state",
        )))
    ));
    assert!(runtime.is_poisoned());
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.object_strong_count(first), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn own_virtual_string_new_admission_drains_pending_runtime_fifo_before_allocation() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let wrapper = object(&mut context, "new String('x')");
    let zero = runtime.intern_property_key("0").unwrap();
    let later = pending_invalid(&runtime);
    assert!(matches!(
        runtime.get_own_property_owned(&wrapper, &zero),
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
fn own_typed_all_twelve_elements_keep_number_bigint_and_default_flags() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let zero = runtime.intern_property_key("0").unwrap();
    for (source, expected) in [
        ("new Uint8ClampedArray([255])", Value::Int(255)),
        ("new Int8Array([-128])", Value::Int(-128)),
        ("new Uint8Array([255])", Value::Int(255)),
        ("new Int16Array([-32768])", Value::Int(-32768)),
        ("new Uint16Array([65535])", Value::Int(65535)),
        ("new Int32Array([-2147483648])", Value::Int(i32::MIN)),
        (
            "new Uint32Array([4294967295])",
            Value::Float(4_294_967_295.0),
        ),
        (
            "new BigInt64Array([-9223372036854775808n])",
            Value::BigInt(JsBigInt::from(i64::MIN)),
        ),
        (
            "new BigUint64Array([18446744073709551615n])",
            Value::BigInt(JsBigInt::from(u64::MAX)),
        ),
        ("new Float16Array([1.5])", Value::Float(1.5)),
        ("new Float32Array([-0])", Value::Float(-0.0)),
        ("new Float64Array([NaN])", Value::Float(f64::NAN)),
    ] {
        let view = object(&mut context, source);
        let public = runtime.get_own_property(&view, &zero).unwrap().unwrap();
        let CompleteOrdinaryPropertyDescriptor::Data {
            value,
            writable,
            enumerable,
            configurable,
        } = public
        else {
            panic!("typed descriptor")
        };
        assert!(value.same_value(&expected), "{source}");
        assert!(writable && enumerable && configurable);
        let owned = runtime
            .get_own_property_owned(&view, &zero)
            .unwrap()
            .unwrap();
        let value = owned.into_data_value().unwrap();
        assert!(
            runtime
                .root_and_release_jsvalue(value)
                .unwrap()
                .same_value(&expected),
            "{source}"
        );
    }
}

#[test]
fn own_typed_canonical_indices_are_terminal_and_shape_errors_stay_late() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let view = object(&mut context, "new Uint8Array([7])");
    let label = runtime.intern_property_key("label").unwrap();
    store(
        &runtime,
        &view,
        &label,
        PropertySlot::Data(RawValue::Int(9)),
    );
    for spelling in [
        "-0",
        "1.5",
        "NaN",
        "Infinity",
        "-1",
        "2",
        "9007199254740992",
    ] {
        let key = runtime.intern_property_key(spelling).unwrap();
        assert!(
            runtime
                .get_own_property_owned(&view, &key)
                .unwrap()
                .is_none(),
            "{spelling}"
        );
    }
    for spelling in ["00", "1e0"] {
        let key = runtime.intern_property_key(spelling).unwrap();
        assert!(
            runtime
                .define_own_property(
                    &view,
                    &key,
                    &data_descriptor(Value::Int(11), true, true, true)
                )
                .unwrap()
        );
        let owned = runtime
            .get_own_property_owned(&view, &key)
            .unwrap()
            .unwrap();
        assert!(matches!(data_value(&owned), RawValue::Int(11)));
    }
    let symbol = runtime.new_symbol(None).unwrap();
    let key = PropertyKey::try_from(&symbol).unwrap();
    store(&runtime, &view, &key, PropertySlot::Data(RawValue::Int(12)));
    let owned = runtime
        .get_own_property_owned(&view, &key)
        .unwrap()
        .unwrap();
    assert!(matches!(data_value(&owned), RawValue::Int(12)));
    let slots = {
        let mut state = runtime.0.state.borrow_mut();
        std::mem::replace(
            &mut state.heap.object_mut(view.object_id()).unwrap().slots,
            Slots::from_vec(vec![]),
        )
    };
    let zero = runtime.intern_property_key("0").unwrap();
    let numeric = runtime.get_own_property_owned(&view, &zero);
    let stored = runtime.get_own_property_owned(&view, &label);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .object_mut(view.object_id())
        .unwrap()
        .slots = slots;
    assert!(matches!(
        data_value(&numeric.unwrap().unwrap()),
        RawValue::Int(7)
    ));
    assert!(matches!(
        stored,
        Err(RuntimeError::Invariant("object property slot was missing"))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn own_shared_typed_word_leaves_state_once_without_heap_root_or_reselection() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let view = object(
        &mut context,
        "(function(){var a=new Uint16Array(new SharedArrayBuffer(4));a[0]=48879;return a})()",
    );
    let zero = runtime.intern_property_key("0").unwrap();
    let word = {
        let mut state = runtime.0.state.borrow_mut();
        let ObjectPayload::TypedArray(data) = &state.heap.object(view.object_id()).unwrap().payload
        else {
            panic!("typed payload")
        };
        let buffer = data.view.buffer;
        let view_count = state.heap.object_strong_count(view.object_id()).unwrap();
        let buffer_count = state.heap.object_strong_count(buffer).unwrap();
        let OwnPropertySelection::Shared(word) = state
            .select_own_property(&runtime.0.poisoned, view.object_id(), zero.atom())
            .unwrap()
        else {
            panic!("shared bytes must leave State")
        };
        assert_eq!(
            state.heap.object_strong_count(view.object_id()),
            Ok(view_count)
        );
        assert_eq!(state.heap.object_strong_count(buffer), Ok(buffer_count));
        word
    };
    let word = word.read().unwrap();
    let record = runtime
        .0
        .state
        .borrow_mut()
        .own_selected_property_descriptor(&runtime.0.poisoned, ReadyOwnProperty::TypedWord(word))
        .unwrap();
    let owned = OwnedCompletePropertyDescriptor::from_owned_record(&runtime, record);
    assert!(matches!(data_value(&owned), RawValue::Int(48_879)));
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn own_resizable_and_detached_typed_bounds_are_revalidated_on_each_read() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let view = object(
        &mut context,
        "var ownBuffer=new ArrayBuffer(4,{maxByteLength:8});var ownView=new Uint16Array(ownBuffer);ownView[1]=65535;ownView",
    );
    let one = runtime.intern_property_key("1").unwrap();
    let owned = runtime
        .get_own_property_owned(&view, &one)
        .unwrap()
        .unwrap();
    assert!(matches!(data_value(&owned), RawValue::Int(65535)));
    let _ = context.eval("ownBuffer.resize(2)").unwrap();
    assert!(
        runtime
            .get_own_property_owned(&view, &one)
            .unwrap()
            .is_none()
    );
    let _ = context.eval("ownBuffer.resize(6);ownView[1]=9").unwrap();
    let owned = runtime
        .get_own_property_owned(&view, &one)
        .unwrap()
        .unwrap();
    assert!(matches!(data_value(&owned), RawValue::Int(9)));
    let _ = context.eval("ownBuffer.transfer()").unwrap();
    assert!(
        runtime
            .get_own_property_owned(&view, &one)
            .unwrap()
            .is_none()
    );
}
