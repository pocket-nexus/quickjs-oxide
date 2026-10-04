use super::*;
use crate::engine::{
    api::Runtime,
    atom::{AtomError, AtomIdx},
    heap::{RawId, runtime::DeferredRefOp},
    value::{
        JsString,
        bigint::{JsBigInt, MAX_BIGINT_BITS},
    },
};

fn converted(result: NativeConversion<Atom>) -> Atom {
    match result {
        NativeConversion::Value(atom) => atom,
        NativeConversion::Throw(_) => panic!("unexpected conversion throw"),
    }
}

#[test]
fn exact_numeric_and_utf16_keys_consume_inputs_without_runtime_roots() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let runtime_owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    for (input, spelling, immediate) in [
        (JsValue::Undefined, "undefined", false),
        (JsValue::Null, "null", false),
        (JsValue::Bool(true), "true", false),
        (JsValue::Int(-3), "-3", false),
        (JsValue::Float(-0.0), "0", true),
        (JsValue::Int(i32::MAX), "2147483647", true),
        (JsValue::Float(2147483648.0), "2147483648", false),
        (JsValue::Float(1e21), "1e+21", false),
        (JsValue::Float(f64::NAN), "NaN", false),
        (JsValue::ShortBigInt(-17), "-17", false),
    ] {
        let atom = converted(
            state
                .property_key_from_primitive_jsvalue(&runtime.0.poisoned, context.realm, input)
                .unwrap(),
        );
        assert_eq!(
            state.atoms.to_js_string(atom).unwrap(),
            JsString::from_static(spelling)
        );
        assert_eq!(atom.is_immediate_integer(), immediate, "{spelling}");
        state.atoms.release(atom).unwrap();
    }
    let text = JsString::try_from_utf16([0xd800, 0x0061, 0x0000, 0xdc00]).unwrap();
    let id = state.heap.allocate_string(text.clone()).unwrap();
    let atom = converted(
        state
            .property_key_from_primitive_jsvalue(
                &runtime.0.poisoned,
                context.realm,
                JsValue::String(id),
            )
            .unwrap(),
    );
    assert_eq!(state.atoms.to_js_string(atom).unwrap(), text);
    assert!(
        state.heap.string(id).is_err(),
        "the suffix consumes its input edge"
    );
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
    state.atoms.release(atom).unwrap();
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), runtime_owners);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn symbol_identity_keeps_a_checked_independent_key_role() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("same spelling")))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(symbol.atom()).unwrap();
    let input = state.dup_jsvalue(&JsValue::Symbol(index)).unwrap();
    let atom = converted(
        state
            .property_key_from_primitive_jsvalue(&runtime.0.poisoned, context.realm, input)
            .unwrap(),
    );
    assert_eq!(atom, symbol.atom());
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(2));
    state.atoms.release(atom).unwrap();
    let input = state.dup_jsvalue(&JsValue::Symbol(index)).unwrap();
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    let result =
        state.property_key_from_primitive_jsvalue(&runtime.0.poisoned, context.realm, input);
    let remaining = state.atoms.resolve(symbol.atom()).unwrap().ref_count;
    state.atoms.set_ref_count_for_test(index, 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Atom(AtomError::RefCountOverflow(_)))
    ));
    assert_eq!(
        remaining,
        Some(u32::MAX - 1),
        "failed key retain still consumes its input"
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn string_intern_overflow_retires_input_without_poisoning() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let key = runtime.intern_property_key("blocked-key").unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let index = AtomIdx::from_raw(key.atom().raw());
    let id = state
        .heap
        .allocate_string(JsString::from_static("blocked-key"))
        .unwrap();
    state.atoms.set_ref_count_for_test(index, u32::MAX);
    let result = state.property_key_from_primitive_jsvalue(
        &runtime.0.poisoned,
        context.realm,
        JsValue::String(id),
    );
    let remaining = state.atoms.resolve(key.atom()).unwrap().ref_count;
    state.atoms.set_ref_count_for_test(index, 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Atom(AtomError::RefCountOverflow(_)))
    ));
    assert_eq!(remaining, Some(u32::MAX));
    assert!(state.heap.string(id).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn object_reply_rejection_consumes_only_the_suffix_owned_edge() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let input = state
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let result =
        state.property_key_from_primitive_jsvalue(&runtime.0.poisoned, context.realm, input);
    assert_eq!(
        result.err(),
        Some(RuntimeError::Invariant(
            "property key conversion received an object"
        ))
    );
    assert_eq!(state.heap.object_strong_count(object.object_id()), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn native_range_error_uses_defining_realm_and_retires_bigint_input() {
    let runtime = Runtime::new();
    let _caller = runtime.new_context().unwrap();
    let defining = runtime.new_context().unwrap();
    let extended = JsBigInt::one()
        .shl(&JsBigInt::from(MAX_BIGINT_BITS - 1))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    // The signed allocation envelope can hold this value while stringification
    // rejects the extra signed limb. The oracle is the existing payload limit.
    assert!(extended.exceeds_allocation_limit());
    let input = state.heap.allocate_bigint(extended).unwrap();
    let NativeConversion::Throw(JsValue::Object(error)) = state
        .property_key_from_primitive_jsvalue(
            &runtime.0.poisoned,
            defining.realm,
            JsValue::BigInt(input),
        )
        .unwrap()
    else {
        panic!("expected native RangeError");
    };
    assert!(state.heap.bigint(input).is_err());
    let prototype = state
        .heap
        .context(defining.realm)
        .unwrap()
        .native_error_prototypes[crate::engine::api::error::NativeErrorKind::Range.index()];
    assert_eq!(
        state
            .heap
            .shape(state.heap.object(error).unwrap().shape)
            .unwrap()
            .prototype(),
        prototype
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(error))
        .unwrap();
    assert!(!runtime.is_poisoned());
}

#[test]
fn fatal_invalid_input_retirement_quarantines_before_followup_traversal() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let id = state
        .heap
        .allocate_string(JsString::from_static("quarantined-reply-key"))
        .unwrap();
    state.heap.set_strong_count_for_test(RawId::String(id), 0);
    let result = state.property_key_from_primitive_jsvalue(
        &runtime.0.poisoned,
        context.realm,
        JsValue::String(id),
    );
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert!(
        state.heap.strong_count(RawId::String(id)) == Ok(0),
        "the invalid zero-count slot is not traversed again after quarantine"
    );
}

#[test]
fn admitted_state_suffix_does_not_add_an_input_kind_coordinator_drain() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
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
    {
        let mut state = runtime.0.state.borrow_mut();
        let input = state
            .heap
            .allocate_string(JsString::from_static("admitted-child-key"))
            .unwrap();
        let atom = converted(
            state
                .property_key_from_primitive_jsvalue(
                    &runtime.0.poisoned,
                    context.realm,
                    JsValue::String(input),
                )
                .unwrap(),
        );
        state.atoms.release(atom).unwrap();
        assert!(runtime.0.deferred_references.has_pending());
        assert!(!runtime.is_poisoned());
    }
    assert!(
        runtime.operation().is_err(),
        "the next genuine entry still admits FIFO first"
    );
    assert!(runtime.is_poisoned());
}
