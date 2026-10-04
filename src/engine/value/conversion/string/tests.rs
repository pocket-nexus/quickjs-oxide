use super::*;
use crate::engine::{
    api::runtime::Runtime,
    heap::{RawId, runtime::DeferredRefOp},
    value::{Value, bigint::JsBigInt},
};

#[test]
fn tostring_state_all_primitive_representations_share_terminal_conversion() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    for (value, expected) in [
        (Value::Undefined, "undefined"),
        (Value::Null, "null"),
        (Value::Bool(true), "true"),
        (Value::Bool(false), "false"),
        (Value::Int(i32::MIN), "-2147483648"),
        (Value::Float(-0.0), "0"),
        (Value::Float(f64::NAN), "NaN"),
        (
            Value::BigInt(JsBigInt::from(i64::MIN)),
            "-9223372036854775808",
        ),
        (
            Value::BigInt(JsBigInt::from(i128::MAX)),
            "170141183460469231731687303715884105727",
        ),
        (Value::String(JsString::from_static("text")), "text"),
    ] {
        let input = runtime.into_jsvalue(value).unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        let NativeConversion::Value(result) = state
            .finish_string_value(
                &runtime.0.poisoned,
                context.realm,
                Completion::Return(input),
            )
            .unwrap()
        else {
            panic!("primitive ToString value");
        };
        assert_eq!(result, JsString::try_from_utf8(expected).unwrap());
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn tostring_state_linearizes_alias_without_replacing_arena_id() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let leaf = JsString::try_from_utf16([0xd800, 0x0061].repeat(1024)).unwrap();
    let rope = leaf.try_concat(&leaf).unwrap();
    assert!(!rope.is_flat());
    let input = runtime.into_jsvalue(Value::String(rope.clone())).unwrap();
    let JsValue::String(id) = input else {
        panic!("string input");
    };
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state.heap.retain_string(id).unwrap();
    let before = state.heap.counts().string_nodes;
    let NativeConversion::Value(result) = state
        .finish_string_value(
            &runtime.0.poisoned,
            context.realm,
            Completion::Return(JsValue::String(id)),
        )
        .unwrap()
    else {
        panic!("string ToString value");
    };
    // Linearization returns the cached flat handle; the original rope and
    // arena node keep their representation and identity.
    assert!(result.is_flat());
    assert!(!rope.is_flat());
    assert!(state.heap.string(id).unwrap().same_representation(&rope));
    assert!(result.same_representation(&rope.linearize()));
    assert!(result.same_representation(&state.heap.string(id).unwrap().linearize()));
    assert_eq!(state.heap.counts().string_nodes, before);
    assert_eq!(state.heap.strong_count(RawId::String(id)), Ok(1));
    assert_eq!(result.code_unit_at(0), Some(0xd800));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(id))
        .unwrap();
}

#[test]
fn tostring_private_child_defers_fifo_failure_to_actual_public_admission() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let symbol = runtime
        .into_jsvalue(Value::Symbol(runtime.new_symbol(None).unwrap()))
        .unwrap();
    let stale = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    runtime
        .0
        .state
        .borrow_mut()
        .release_jsvalue(JsValue::Object(stale))
        .unwrap();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(stale));
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(later));

    // The old error-only native Error adapter admitted/drained here. The
    // complete State conversion now keeps the enclosing operation's boundary.
    let NativeConversion::Throw(JsValue::Object(error)) = runtime
        .native_to_js_string_jsvalue(context.realm, symbol)
        .unwrap()
    else {
        panic!("Symbol ToString must produce its TypeError");
    };
    assert!(!runtime.is_poisoned());
    assert!(runtime.0.deferred_references.has_pending());
    {
        let mut state = runtime.0.state.borrow_mut();
        let expected = state
            .heap
            .context(context.realm)
            .unwrap()
            .native_error_prototypes[NativeErrorKind::Type.index()];
        let shape = state.heap.object(error).unwrap().shape;
        assert_eq!(state.heap.shape(shape).unwrap().prototype(), expected);
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(error))
            .unwrap();
    }
    assert!(matches!(
        runtime.new_object(None),
        Err(RuntimeError::Heap(_))
    ));
    assert!(runtime.is_poisoned());
    assert!(runtime.0.deferred_references.has_pending());
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(later),
        Ok(1)
    );
}

#[test]
fn tostring_state_error_publication_failure_quarantines_original_input_before_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let symbol = runtime.new_symbol(None).unwrap();
    let input = runtime
        .into_jsvalue(Value::Symbol(symbol.try_clone().unwrap()))
        .unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.counts().object_nodes;
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
    {
        let mut suffix_owner =
            OwnedValueGuard::new(&mut state, &runtime.0.poisoned, JsValue::Object(suffix));
        let (state, _) = suffix_owner.parts();
        let result = state.finish_string_value(
            &runtime.0.poisoned,
            context.realm,
            Completion::Return(input),
        );
        assert!(matches!(
            result,
            Err(RuntimeError::Heap(
                crate::engine::heap::HeapError::Invariant(
                    "finalization count disagrees with its queue/cycle state"
                )
            ))
        ));
        assert!(runtime.is_poisoned());
    }
    assert_eq!(state.heap.counts().object_nodes, before + 1);
    assert_eq!(
        state.atoms.resolve(symbol.atom()).unwrap().ref_count,
        Some(2)
    );
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}
