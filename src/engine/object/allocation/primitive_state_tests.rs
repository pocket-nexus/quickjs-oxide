use super::*;
use crate::engine::atom::AtomError;
use crate::engine::heap::RawId;
use crate::engine::heap::runtime::owned_values::OwnedValuesGuard;
use crate::engine::value::bigint::JsBigInt;

fn payload(state: &RuntimeState, object: ObjectId) -> &PrimitiveObjectData {
    let ObjectPayload::Primitive(data) = &state.heap.object(object).unwrap().payload else {
        panic!("primitive object payload");
    };
    data
}

fn queue_invalid_zero(state: &mut RuntimeState, first: ObjectId, later: ObjectId) {
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
}

fn queue_error() -> RuntimeError {
    RuntimeError::Heap(crate::engine::heap::HeapError::Invariant(
        "finalization count disagrees with its queue/cycle state",
    ))
}

#[test]
fn primitive_state_all_representations_share_public_factory_and_consumed_owners() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("wrapper")))
        .unwrap();
    let cases = [
        (PrimitiveKind::Number, Value::Int(i32::MIN)),
        (PrimitiveKind::Number, Value::Float(-0.0)),
        (PrimitiveKind::Number, Value::Float(f64::NAN)),
        (PrimitiveKind::Boolean, Value::Bool(true)),
        (
            PrimitiveKind::String,
            Value::String(JsString::from_static("flat")),
        ),
        (
            PrimitiveKind::Symbol,
            Value::Symbol(symbol.try_clone().unwrap()),
        ),
        (
            PrimitiveKind::BigInt,
            Value::BigInt(JsBigInt::from(i64::MIN)),
        ),
        (
            PrimitiveKind::BigInt,
            Value::BigInt(JsBigInt::from(i128::MAX)),
        ),
    ];
    for (kind, value) in cases {
        let wrapper = runtime
            .new_primitive_object(&prototype, kind, value.try_clone().unwrap())
            .unwrap();
        let input = runtime.into_jsvalue(value).unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        let direct = state
            .new_primitive_object_jsvalue(
                &runtime.0.poisoned,
                prototype.object_id(),
                kind,
                input,
                false,
            )
            .unwrap();
        assert_eq!(state.heap.object_strong_count(direct), Ok(1));
        assert_eq!(
            state
                .heap
                .shape(state.heap.object(direct).unwrap().shape)
                .unwrap()
                .prototype(),
            Some(prototype.object_id())
        );
        match (
            payload(&state, wrapper.object_id()),
            payload(&state, direct),
        ) {
            (PrimitiveObjectData::Number(left), PrimitiveObjectData::Number(right)) => {
                assert_eq!(left.to_bits(), right.to_bits())
            }
            (PrimitiveObjectData::Boolean(left), PrimitiveObjectData::Boolean(right)) => {
                assert_eq!(left, right)
            }
            (PrimitiveObjectData::String(left), PrimitiveObjectData::String(right)) => {
                assert_eq!(
                    state.heap.string(*left).unwrap(),
                    state.heap.string(*right).unwrap()
                );
                assert_eq!(state.heap.strong_count(RawId::String(*right)), Ok(1));
            }
            (PrimitiveObjectData::Symbol(left), PrimitiveObjectData::Symbol(right)) => {
                assert_eq!(left, right)
            }
            (PrimitiveObjectData::ShortBigInt(left), PrimitiveObjectData::ShortBigInt(right)) => {
                assert_eq!(left, right)
            }
            (PrimitiveObjectData::BigInt(left), PrimitiveObjectData::BigInt(right)) => {
                assert_eq!(
                    state.heap.bigint(*left).unwrap(),
                    state.heap.bigint(*right).unwrap()
                );
                assert_eq!(state.heap.strong_count(RawId::BigInt(*right)), Ok(1));
            }
            _ => panic!("same primitive representation"),
        }
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(direct))
            .unwrap();
    }
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .atoms
            .resolve(symbol.atom())
            .unwrap()
            .ref_count,
        Some(1)
    );
}

#[test]
fn primitive_state_flat_string_id_and_length_flags_survive_consuming_input() {
    for configurable in [false, true] {
        let runtime = Runtime::new();
        let prototype = runtime.new_object(None).unwrap();
        let JsValue::String(input) = runtime
            .into_jsvalue(Value::String(JsString::from_static("flat")))
            .unwrap()
        else {
            panic!("string input")
        };
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        state.heap.retain_string(input).unwrap();
        let wrapper = state
            .new_primitive_object_jsvalue(
                &runtime.0.poisoned,
                prototype.object_id(),
                PrimitiveKind::String,
                JsValue::String(input),
                configurable,
            )
            .unwrap();
        assert!(
            matches!(payload(&state, wrapper), PrimitiveObjectData::String(stored) if *stored == input)
        );
        assert_eq!(
            state.heap.strong_count(RawId::String(input)),
            Ok(2),
            "kept source plus wrapper payload"
        );
        let object = state.heap.object(wrapper).unwrap();
        let shape = state.heap.shape(object.shape).unwrap();
        assert_eq!(shape.entries().len(), 1);
        assert_eq!(
            shape.entries()[0].atom.raw(),
            state
                .pinned_atoms
                .get(crate::engine::atom::pinned::PinnedAtom::Length)
                .raw()
        );
        assert_eq!(
            shape.entries()[0].flags,
            PropertyFlags::data(false, false, configurable)
        );
        assert!(matches!(
            &object.slots[0],
            PropertySlot::Data(RawValue::Int(4))
        ));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(wrapper))
            .unwrap();
        assert_eq!(state.heap.strong_count(RawId::String(input)), Ok(1));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(input))
            .unwrap();
    }
}

#[test]
fn primitive_state_rope_linearization_keeps_shared_source_id_and_uses_new_flat_node() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let left = JsString::try_from_utf8(&"a".repeat(8193)).unwrap();
    let right = JsString::try_from_utf16(std::iter::repeat_n(0xd800, 8193)).unwrap();
    let rope = left.try_concat(&right).unwrap();
    assert!(!rope.is_flat());
    let JsValue::String(input) = runtime.into_jsvalue(Value::String(rope.clone())).unwrap() else {
        panic!("rope input")
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state.heap.retain_string(input).unwrap();
    let wrapper = state
        .new_primitive_object_jsvalue(
            &runtime.0.poisoned,
            prototype.object_id(),
            PrimitiveKind::String,
            JsValue::String(input),
            false,
        )
        .unwrap();
    let PrimitiveObjectData::String(stored) = *payload(&state, wrapper) else {
        panic!("string wrapper")
    };
    assert_ne!(stored, input);
    assert!(state.heap.string(input).unwrap().same_representation(&rope));
    assert!(!state.heap.string(input).unwrap().is_flat());
    assert!(state.heap.string(stored).unwrap().is_flat());
    assert_eq!(state.heap.string(stored).unwrap(), &rope);
    assert_eq!(state.heap.strong_count(RawId::String(input)), Ok(1));
    assert_eq!(state.heap.strong_count(RawId::String(stored)), Ok(1));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(wrapper))
        .unwrap();
    assert!(state.heap.string(stored).is_err());
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(input))
        .unwrap();
}

#[test]
fn primitive_state_rope_producer_cleanup_quarantines_normalized_input_before_allocation() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let left = JsString::try_from_utf8(&"a".repeat(8193)).unwrap();
    let right = JsString::try_from_utf8(&"b".repeat(8193)).unwrap();
    let rope = left.try_concat(&right).unwrap();
    assert!(!rope.is_flat());
    let JsValue::String(input) = runtime.into_jsvalue(Value::String(rope.clone())).unwrap() else {
        panic!("rope input")
    };
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state.heap.retain_string(input).unwrap();
    let before = state.heap.counts();
    queue_invalid_zero(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.new_primitive_object_jsvalue(
                &runtime.0.poisoned,
                prototype.object_id(),
                PrimitiveKind::String,
                JsValue::String(input),
                false,
            ),
            Err(queue_error())
        );
        assert!(runtime.is_poisoned());
    }
    assert_eq!(state.heap.counts().object_nodes, before.object_nodes);
    assert_eq!(
        state.heap.counts().string_nodes,
        before.string_nodes + 1,
        "normalized producer remains quarantined; no wrapper was allocated"
    );
    assert_eq!(state.heap.strong_count(RawId::String(input)), Ok(1));
    assert!(state.heap.string(input).unwrap().same_representation(&rope));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn primitive_public_value_rope_linearizes_before_arena_conversion() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let left = JsString::try_from_utf8(&"a".repeat(8193)).unwrap();
    let right = JsString::try_from_utf8(&"b".repeat(8193)).unwrap();
    let rope = left.try_concat(&right).unwrap();
    assert!(!rope.is_flat());
    let before = runtime.0.state.borrow().heap.counts().string_nodes;
    let wrapper = runtime
        .new_primitive_object(
            &prototype,
            PrimitiveKind::String,
            Value::String(rope.clone()),
        )
        .unwrap();
    let state = runtime.0.state.borrow();
    let PrimitiveObjectData::String(stored) = *payload(&state, wrapper.object_id()) else {
        panic!("string wrapper")
    };
    assert!(state.heap.string(stored).unwrap().is_flat());
    assert!(
        state
            .heap
            .string(stored)
            .unwrap()
            .same_representation(&rope.linearize())
    );
    assert_eq!(state.heap.counts().string_nodes, before + 1);
    assert_eq!(state.heap.strong_count(RawId::String(stored)), Ok(1));
}

#[test]
fn primitive_public_value_admits_prototype_then_payload_before_class_selection() {
    let runtime = Runtime::new();
    let local_prototype = runtime.new_object(None).unwrap();
    let foreign = Runtime::new();
    let foreign_prototype = foreign.new_object(None).unwrap();
    let symbol = foreign
        .new_symbol(Some(JsString::from_static("foreign-payload")))
        .unwrap();
    for (prototype, expected) in [
        (&foreign_prototype, "primitive prototype"),
        (&local_prototype, "primitive wrapper payload"),
    ] {
        assert!(matches!(
            runtime.new_primitive_object(
                prototype,
                PrimitiveKind::Number,
                Value::Symbol(symbol.try_clone().unwrap()),
            ),
            Err(RuntimeError::WrongRuntime(role)) if role == expected
        ));
        assert_eq!(
            foreign
                .0
                .state
                .borrow()
                .atoms
                .resolve(symbol.atom())
                .unwrap()
                .ref_count,
            Some(1),
            "rejected public payload root is consumed once"
        );
        assert!(!runtime.is_poisoned());
        assert!(!foreign.is_poisoned());
    }
}

#[test]
fn primitive_state_symbol_retention_is_single_and_max_failure_is_recoverable() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("symbol-payload")))
        .unwrap();
    let wrapper = runtime
        .new_primitive_object(
            &prototype,
            PrimitiveKind::Symbol,
            Value::Symbol(symbol.try_clone().unwrap()),
        )
        .unwrap();
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .atoms
            .resolve(symbol.atom())
            .unwrap()
            .ref_count,
        Some(2)
    );
    drop(wrapper);
    let input = runtime
        .into_jsvalue(Value::Symbol(symbol.try_clone().unwrap()))
        .unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    let before = state.heap.counts().object_nodes;
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    let result = state.new_primitive_object_jsvalue(
        &runtime.0.poisoned,
        prototype.object_id(),
        PrimitiveKind::Symbol,
        input,
        false,
    );
    let observed = state.atoms.resolve(symbol.atom()).unwrap().ref_count;
    state.atoms.set_ref_count_for_test(index, 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Atom(AtomError::RefCountOverflow(_)))
    ));
    assert_eq!(
        observed,
        Some(u32::MAX - 1),
        "failed payload retain then consumed input release"
    );
    assert_eq!(state.heap.counts().object_nodes, before);
    assert!(!runtime.is_poisoned());
}

#[test]
fn primitive_state_shape_failure_precedes_symbol_payload_retain_failure() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let warm = runtime
        .new_primitive_object(&prototype, PrimitiveKind::Number, Value::Int(0))
        .unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("prefix")))
        .unwrap();
    let input = runtime
        .into_jsvalue(Value::Symbol(symbol.try_clone().unwrap()))
        .unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let shape = state.heap.object(warm.object_id()).unwrap().shape;
    let original_shape_count = state.heap.shape_strong_count(shape).unwrap();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Shape(shape), u32::MAX);
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    let result = state.new_primitive_object_jsvalue(
        &runtime.0.poisoned,
        prototype.object_id(),
        PrimitiveKind::Symbol,
        input,
        false,
    );
    let observed_symbol = state.atoms.resolve(symbol.atom()).unwrap().ref_count;
    state
        .heap
        .set_strong_count_for_test(RawId::Shape(shape), original_shape_count);
    state.atoms.set_ref_count_for_test(index, 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(
            crate::engine::heap::HeapError::Overflow { .. }
        ))
    ));
    assert_eq!(observed_symbol, Some(u32::MAX - 1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn primitive_state_slot_and_payload_atom_retains_roll_back_without_publication() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("two-object-edges")))
        .unwrap();
    let key = runtime.intern_property_key("slot").unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    let before = state.heap.counts().object_nodes;
    state.atoms.set_ref_count_for_test(index, u32::MAX - 1);
    let result = state.allocate_object_with_layout(
        &runtime.0.poisoned,
        Some(prototype.object_id()),
        &[ShapeEntry {
            atom: AtomIdx::from_raw(key.atom().raw()),
            flags: PropertyFlags::data(true, true, true),
        }],
        vec![PropertySlot::Data(RawValue::Symbol(index))],
        |shape, slots| {
            ObjectData::primitive(shape, slots, PrimitiveObjectData::Symbol(symbol.atom()))
        },
    );
    let observed = state.atoms.resolve(symbol.atom()).unwrap().ref_count;
    state.atoms.set_ref_count_for_test(index, 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Atom(AtomError::RefCountOverflow(_)))
    ));
    assert_eq!(observed, Some(u32::MAX - 1));
    assert_eq!(
        state.atoms.resolve(key.atom()).unwrap().ref_count,
        Some(1),
        "failed layout retires its key owner"
    );
    assert_eq!(state.heap.counts().object_nodes, before);
    assert!(!runtime.is_poisoned());
}

#[test]
fn primitive_state_payload_and_private_home_atom_edges_use_canonical_finalizer() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("payload")))
        .unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let brand = state
        .atoms
        .new_private_symbol_js_string(Some(JsString::from_static("brand")))
        .unwrap();
    let object = state
        .allocate_object_with_layout(
            &runtime.0.poisoned,
            Some(prototype.object_id()),
            &[],
            Vec::new(),
            |shape, slots| {
                let mut data =
                    ObjectData::primitive(shape, slots, PrimitiveObjectData::Symbol(symbol.atom()));
                data.private_brand_home = Some(brand);
                data
            },
        )
        .unwrap();
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(2)
    );
    assert_eq!(state.atoms.resolve(brand).unwrap().ref_count, Some(2));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
        .unwrap();
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(1)
    );
    assert_eq!(state.atoms.resolve(brand).unwrap().ref_count, Some(1));
    state.atoms.release(brand).unwrap();
}

#[test]
fn primitive_state_published_symbol_wrapper_quarantines_input_and_suffix() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let _warm = runtime
        .new_primitive_object(&prototype, PrimitiveKind::Number, Value::Int(0))
        .unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("published-symbol")))
        .unwrap();
    let input = runtime
        .into_jsvalue(Value::Symbol(symbol.try_clone().unwrap()))
        .unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let objects_before = state.heap.counts().object_nodes;
    queue_invalid_zero(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.new_primitive_object_jsvalue(
                &runtime.0.poisoned,
                prototype.object_id(),
                PrimitiveKind::Symbol,
                input,
                false
            ),
            Err(queue_error())
        );
        assert!(runtime.is_poisoned());
    }
    assert_eq!(state.heap.counts().object_nodes, objects_before + 1);
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(3),
        "public Symbol, published payload, quarantined input producer"
    );
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn primitive_state_symbol_retain_rollback_cleanup_quarantines_before_input_retirement() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let _warm = runtime
        .new_primitive_object(&prototype, PrimitiveKind::Number, Value::Int(0))
        .unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("failed-symbol-retain")))
        .unwrap();
    let input = runtime
        .into_jsvalue(Value::Symbol(symbol.try_clone().unwrap()))
        .unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    let objects_before = state.heap.counts().object_nodes;
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    queue_invalid_zero(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.new_primitive_object_jsvalue(
                &runtime.0.poisoned,
                prototype.object_id(),
                PrimitiveKind::Symbol,
                input,
                false
            ),
            Err(queue_error())
        );
        assert!(runtime.is_poisoned());
    }
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(u32::MAX),
        "no input release after destructive rollback cleanup"
    );
    state.atoms.set_ref_count_for_test(index, 2);
    assert_eq!(state.heap.counts().object_nodes, objects_before);
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
}

#[test]
fn primitive_state_string_length_retain_failure_reclaims_partial_wrapper_and_retries() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let _warm = runtime
        .new_primitive_object(&prototype, PrimitiveKind::Number, Value::Int(0))
        .unwrap();
    let JsValue::String(input) = runtime
        .into_jsvalue(Value::String(JsString::from_static("length")))
        .unwrap()
    else {
        panic!("String input")
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state.heap.retain_string(input).unwrap();
    let prototype_count = state
        .heap
        .object_strong_count(prototype.object_id())
        .unwrap();
    let objects_before = state.heap.counts().object_nodes;
    // The empty layout is already cached. Initial object publication succeeds;
    // adding length must create a successor shape and retain its prototype.
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype.object_id()), u32::MAX);
    let result = state.new_primitive_object_jsvalue(
        &runtime.0.poisoned,
        prototype.object_id(),
        PrimitiveKind::String,
        JsValue::String(input),
        false,
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(prototype.object_id()), prototype_count);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(
            crate::engine::heap::HeapError::Overflow { .. }
        ))
    ));
    assert_eq!(state.heap.counts().object_nodes, objects_before);
    assert_eq!(state.heap.strong_count(RawId::String(input)), Ok(1));
    assert!(!runtime.is_poisoned());
    state.heap.retain_string(input).unwrap();
    let retried = state
        .new_primitive_object_jsvalue(
            &runtime.0.poisoned,
            prototype.object_id(),
            PrimitiveKind::String,
            JsValue::String(input),
            false,
        )
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(retried))
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(input))
        .unwrap();
}

#[test]
fn primitive_state_heap_payload_retain_overflow_is_recoverable_before_publication() {
    for bigint in [false, true] {
        let runtime = Runtime::new();
        let prototype = runtime.new_object(None).unwrap();
        let value = if bigint {
            Value::BigInt(JsBigInt::from(i128::MAX))
        } else {
            Value::String(JsString::from_static("flat"))
        };
        let input = runtime.into_jsvalue(value).unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        let kept = state.dup_jsvalue(&input).unwrap();
        let raw_id = match &input {
            JsValue::String(id) => RawId::String(*id),
            JsValue::BigInt(id) => RawId::BigInt(*id),
            _ => panic!("heap primitive"),
        };
        let objects_before = state.heap.counts().object_nodes;
        state.heap.set_strong_count_for_test(raw_id, u32::MAX);
        let result = state.new_primitive_object_jsvalue(
            &runtime.0.poisoned,
            prototype.object_id(),
            if bigint {
                PrimitiveKind::BigInt
            } else {
                PrimitiveKind::String
            },
            input,
            false,
        );
        let observed = state.heap.strong_count(raw_id);
        state.heap.set_strong_count_for_test(raw_id, 1);
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(
                crate::engine::heap::HeapError::Overflow { .. }
            ))
        ));
        assert_eq!(
            observed,
            Ok(u32::MAX),
            "existing immortal sentinel prevents input decrement"
        );
        assert_eq!(state.heap.counts().object_nodes, objects_before);
        assert!(!runtime.is_poisoned());
        state
            .release_owned_jsvalue(&runtime.0.poisoned, kept)
            .unwrap();
    }
}

#[test]
fn primitive_public_foreign_prototype_consumes_input_once_and_poison_skips_cleanup() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let prototype = foreign.new_object(None).unwrap();
    let JsValue::String(first) = runtime
        .into_jsvalue(Value::String(JsString::from_static("rejected")))
        .unwrap()
    else {
        panic!("String input")
    };
    assert!(matches!(
        runtime.new_primitive_object_jsvalue(
            &prototype,
            PrimitiveKind::String,
            JsValue::String(first)
        ),
        Err(RuntimeError::WrongRuntime("primitive prototype"))
    ));
    assert!(runtime.0.state.borrow().heap.string(first).is_err());
    let JsValue::String(quarantined) = runtime
        .into_jsvalue(Value::String(JsString::from_static("quarantined")))
        .unwrap()
    else {
        panic!("String input")
    };
    runtime.0.poisoned.set(true);
    assert!(matches!(
        runtime.new_primitive_object_jsvalue(
            &prototype,
            PrimitiveKind::String,
            JsValue::String(quarantined)
        ),
        Err(RuntimeError::Poisoned)
    ));
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .strong_count(RawId::String(quarantined)),
        Ok(1)
    );
}

#[test]
fn primitive_state_class_mismatch_consumes_owned_object_without_allocating_wrapper() {
    let runtime = Runtime::new();
    let prototype = runtime.new_object(None).unwrap();
    let input = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.counts().object_nodes;
    assert_eq!(
        state.new_primitive_object_jsvalue(
            &runtime.0.poisoned,
            prototype.object_id(),
            PrimitiveKind::Number,
            JsValue::Object(input),
            false
        ),
        Err(RuntimeError::Invariant(
            "primitive wrapper class or payload is not implemented yet"
        ))
    );
    assert!(state.heap.object(input).is_err());
    assert_eq!(state.heap.counts().object_nodes, before - 1);
    assert!(!runtime.is_poisoned());
}
