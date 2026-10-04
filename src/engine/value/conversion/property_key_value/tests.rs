use super::*;
use crate::engine::{
    api::{ErrorKind, Runtime},
    heap::{HeapError, RawId, runtime::DeferredRefOp},
    value::{
        JsString,
        bigint::{JsBigInt, MAX_BIGINT_BITS},
    },
};

#[test]
fn key_value_formats_complete_primitive_domain_without_roots_or_atoms() {
    let runtime = Runtime::new();
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let atom_count = state.atoms.len();
    let bigint = state
        .heap
        .allocate_bigint(JsBigInt::from(i128::MAX))
        .unwrap();
    for (value, expected) in [
        (JsValue::Undefined, "undefined"),
        (JsValue::Null, "null"),
        (JsValue::Bool(true), "true"),
        (JsValue::Bool(false), "false"),
        (JsValue::Int(i32::MIN), "-2147483648"),
        (JsValue::Float(-0.0), "0"),
        (JsValue::Float(f64::NAN), "NaN"),
        (JsValue::Float(f64::INFINITY), "Infinity"),
        (JsValue::Float(f64::NEG_INFINITY), "-Infinity"),
        (JsValue::Float(1e21), "1e+21"),
        (JsValue::ShortBigInt(i64::MIN), "-9223372036854775808"),
        (
            JsValue::BigInt(bigint),
            "170141183460469231731687303715884105727",
        ),
    ] {
        let before = state.heap.counts().string_nodes;
        let JsValue::String(result) = state
            .property_key_primitive(&runtime.0.poisoned, value)
            .unwrap()
        else {
            panic!("formatted key is a String value")
        };
        assert_eq!(
            state.heap.string(result).unwrap(),
            &JsString::from_static(expected)
        );
        assert_eq!(state.heap.counts().string_nodes, before + 1);
        assert_eq!(state.heap.strong_count(RawId::String(result)), Ok(1));
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(result))
            .unwrap();
    }
    assert!(
        state.heap.bigint(bigint).is_err(),
        "the long BigInt input is consumed"
    );
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
    assert_eq!(state.atoms.len(), atom_count);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn key_value_string_keeps_utf16_rope_id_and_saturated_owner() {
    let runtime = Runtime::new();
    let leaf = JsString::try_from_utf16([0xd800, 0x0061].repeat(1024)).unwrap();
    let rope = leaf.try_concat(&leaf).unwrap();
    assert!(!rope.is_flat());
    let mut state = runtime.0.state.borrow_mut();
    let input = state.heap.allocate_string(rope.clone()).unwrap();
    let before = state.heap.counts().string_nodes;
    state
        .heap
        .set_strong_count_for_test(RawId::String(input), u32::MAX);
    let JsValue::String(result) = state
        .property_key_primitive(&runtime.0.poisoned, JsValue::String(input))
        .unwrap()
    else {
        panic!("String input transfers unchanged")
    };
    assert_eq!(result, input);
    assert_eq!(state.heap.strong_count(RawId::String(result)), Ok(u32::MAX));
    assert_eq!(state.heap.counts().string_nodes, before);
    assert!(
        state
            .heap
            .string(result)
            .unwrap()
            .same_representation(&rope)
    );
    assert!(!state.heap.string(result).unwrap().is_flat());
    assert_eq!(
        state.heap.string(result).unwrap().code_unit_at(0),
        Some(0xd800)
    );
    state
        .heap
        .set_strong_count_for_test(RawId::String(input), 1);
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(result))
        .unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn key_value_symbol_transfers_identity_without_independent_key_retain() {
    let runtime = Runtime::new();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("value-key")))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    let input = state.dup_jsvalue(&JsValue::Symbol(index)).unwrap();
    let actual = state
        .atoms
        .resolve(symbol.atom())
        .unwrap()
        .ref_count
        .expect("live Symbol atom");
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    let JsValue::Symbol(result) = state
        .property_key_primitive(&runtime.0.poisoned, input)
        .unwrap()
    else {
        panic!("Symbol input transfers unchanged")
    };
    assert_eq!(result, index);
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(u32::MAX)
    );
    state.atoms.set_ref_count_for_test(index, actual);
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Symbol(result))
        .unwrap();
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(actual - 1)
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn key_value_formatting_rejection_retires_input_without_native_error_allocation() {
    let runtime = Runtime::new();
    let extended = JsBigInt::one()
        .shl(&JsBigInt::from(MAX_BIGINT_BITS - 1))
        .unwrap();
    assert!(extended.exceeds_allocation_limit());
    let mut state = runtime.0.state.borrow_mut();
    let input = state.heap.allocate_bigint(extended).unwrap();
    let before = state.heap.counts();
    let result = state.property_key_primitive(&runtime.0.poisoned, JsValue::BigInt(input));
    let Err(RuntimeError::Engine(error)) = result else {
        panic!("formatter Range error")
    };
    assert_eq!(error.kind(), ErrorKind::Range);
    assert_eq!(error.message(), "BigInt is too large to allocate");
    assert!(state.heap.bigint(input).is_err());
    assert_eq!(state.heap.counts().string_nodes, before.string_nodes);
    assert_eq!(state.heap.counts().object_nodes, before.object_nodes);
    assert!(!runtime.is_poisoned());
}

#[test]
fn key_value_object_reply_rejection_consumes_only_its_owned_edge() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let input = state
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let before = state.heap.counts();
    let Err(RuntimeError::Engine(error)) = state.property_key_primitive(&runtime.0.poisoned, input)
    else {
        panic!("Object input must have completed ToPrimitive before this suffix")
    };
    assert_eq!(error.kind(), ErrorKind::Internal);
    assert_eq!(
        error.message(),
        "object ToPrimitive requires an execution context"
    );
    assert_eq!(state.heap.object_strong_count(object.object_id()), Ok(1));
    assert_eq!(state.heap.counts().object_nodes, before.object_nodes);
    assert_eq!(state.heap.counts().string_nodes, before.string_nodes);
    assert!(!runtime.is_poisoned());
}

#[test]
fn key_value_fatal_input_retirement_precedes_output_and_stops_suffix() {
    for reject_format in [false, true] {
        let runtime = Runtime::new();
        let first = runtime.new_object(None).unwrap().into_handle();
        let suffix = runtime.new_object(None).unwrap().into_handle();
        let payload = if reject_format {
            JsBigInt::one()
                .shl(&JsBigInt::from(MAX_BIGINT_BITS - 1))
                .unwrap()
        } else {
            JsBigInt::from(i128::MAX)
        };
        let mut state = runtime.0.state.borrow_mut();
        let input = state.heap.allocate_bigint(payload).unwrap();
        state
            .heap
            .queue_release_for_test(RawId::Object(first))
            .unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(first), 1);
        let before = state.heap.counts().string_nodes;
        let mut suffix_owner =
            OwnedValueGuard::new(&mut state, &runtime.0.poisoned, JsValue::Object(suffix));
        {
            let (borrowed_state, _) = suffix_owner.parts();
            let result =
                borrowed_state.property_key_primitive(&runtime.0.poisoned, JsValue::BigInt(input));
            assert!(
                matches!(
                    result,
                    Err(RuntimeError::Heap(HeapError::Invariant(
                        "finalization count disagrees with its queue/cycle state"
                    )))
                ),
                "actual retirement error wins even if formatting also rejects"
            );
            assert!(runtime.is_poisoned());
            assert_eq!(
                borrowed_state.heap.strong_count(RawId::BigInt(input)),
                Ok(0),
                "input was retired but its queued payload remains quarantined"
            );
            assert!(borrowed_state.heap.has_pending_zero_cleanup());
            assert_eq!(
                borrowed_state.heap.counts().string_nodes,
                before,
                "no output allocation before input retirement"
            );
        }
        drop(suffix_owner);
        assert_eq!(
            state.heap.object_strong_count(suffix),
            Ok(1),
            "outer owner guard skips traversal after poison"
        );
    }
}

#[test]
fn key_value_runtime_adapter_keeps_actual_coordinator_boundary() {
    let runtime = Runtime::new();
    let stale = runtime.new_object(None).unwrap().into_handle();
    runtime
        .0
        .state
        .borrow_mut()
        .release_object_handle(stale)
        .unwrap();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(stale));
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    let value = runtime
        .property_key_primitive(JsValue::Float(-0.0))
        .unwrap();
    let JsValue::String(result) = value else {
        panic!("String key value")
    };
    {
        let mut state = runtime.0.state.borrow_mut();
        assert_eq!(
            state.heap.string(result).unwrap(),
            &JsString::from_static("0")
        );
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(result))
            .unwrap();
    }
    let input = runtime
        .0
        .state
        .borrow_mut()
        .heap
        .allocate_bigint(
            JsBigInt::one()
                .shl(&JsBigInt::from(MAX_BIGINT_BITS - 1))
                .unwrap(),
        )
        .unwrap();
    let error = runtime
        .property_key_primitive(JsValue::BigInt(input))
        .expect_err("conversion Range error reaches the VM adapter unchanged");
    assert_eq!(error.kind(), ErrorKind::Range);
    assert_eq!(error.message(), "BigInt is too large to allocate");
    assert!(runtime.0.state.borrow().heap.bigint(input).is_err());
    assert!(runtime.0.deferred_references.has_pending());
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
    assert!(!runtime.is_poisoned());
    assert!(
        runtime.operation().is_err(),
        "only the genuine caller entry drains FIFO"
    );
    assert!(runtime.is_poisoned());
}
