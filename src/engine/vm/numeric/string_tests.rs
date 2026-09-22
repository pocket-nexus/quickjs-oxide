use super::*;

fn string(runtime: &Runtime, text: &str) -> JsValue {
    allocate_string_jsvalue(runtime, JsString::try_from_utf8(text).unwrap()).unwrap()
}

fn finish(runtime: &Runtime, value: JsValue) -> JsString {
    let JsValue::String(id) = &value else {
        panic!("string result")
    };
    let text = string_payload(runtime, *id).unwrap();
    runtime.release_jsvalue(value).unwrap();
    text
}

#[test]
fn consuming_concat_preserves_unique_node_and_buffer_algorithm() {
    let runtime = Runtime::new();
    let left = string(&runtime, "prefix");
    let JsValue::String(original) = left else {
        unreachable!()
    };
    let right = string(&runtime, "suffix");
    let result = add_primitives(&runtime, left, right).unwrap();
    let reused = matches!(result, JsValue::String(id) if id == original);
    let text = finish(&runtime, result);
    assert!(reused, "consuming concat must keep the unique arena owner");
    assert_eq!(text, JsString::from_static("prefixsuffix"));
}

#[test]
fn consuming_concat_preserves_shared_edges_and_public_payloads() {
    let runtime = Runtime::new();
    let left = string(&runtime, "a");
    let alias = runtime.dup_jsvalue(&left).unwrap();
    let result = add_primitives(&runtime, left, string(&runtime, "b")).unwrap();
    let text = finish(&runtime, result);
    let original = finish(&runtime, alias);
    assert_eq!(text, JsString::from_static("ab"));
    assert_eq!(original, JsString::from_static("a"));

    let left = string(&runtime, "same");
    let right = runtime.dup_jsvalue(&left).unwrap();
    let result = add_primitives(&runtime, left, right).unwrap();
    assert_eq!(finish(&runtime, result), JsString::from_static("samesame"));

    let public = JsString::try_from_utf8("public").unwrap();
    let left = allocate_string_jsvalue(&runtime, public.clone()).unwrap();
    let result = add_primitives(&runtime, left, string(&runtime, "!")).unwrap();
    assert_eq!(finish(&runtime, result), JsString::from_static("public!"));
    assert_eq!(public, JsString::from_static("public"));
}

#[test]
fn consuming_concat_preserves_empty_representation_and_length_error() {
    let runtime = Runtime::new();
    for (left, right) in [("", "x"), ("x", "")] {
        let result =
            add_primitives(&runtime, string(&runtime, left), string(&runtime, right)).unwrap();
        assert_eq!(finish(&runtime, result), JsString::from_static("x"));
    }
    let mut huge = JsString::try_from_utf8(&"x".repeat(8193)).unwrap();
    for _ in 0..16 {
        huge = huge.try_concat(&huge).unwrap();
    }
    let left = allocate_string_jsvalue(&runtime, huge.clone()).unwrap();
    let right = allocate_string_jsvalue(&runtime, huge).unwrap();
    let error = add_primitives(&runtime, left, right).unwrap_err();
    assert_eq!(error.message(), "string too long");
}

#[cfg(feature = "profiling")]
#[test]
fn consuming_concat_reservation_failure_releases_both_inputs() {
    let runtime = Runtime::new();
    let left = string(&runtime, "a");
    let right = string(&runtime, "b");
    crate::engine::value::fail_next_concat_reservation_for_test();
    let result = add_primitives(&runtime, left, right);
    if let Ok(value) = result {
        runtime.release_jsvalue(value).unwrap();
        panic!("unique concat must reach the fallible reservation");
    }
    // Runtime teardown checks that both consumed input nodes were released.
}

#[test]
fn consuming_concat_keeps_pending_release_boundary() {
    {
        let runtime = Runtime::new();
        let left = string(&runtime, "a");
        let JsValue::String(original) = left else {
            unreachable!()
        };
        let queued = runtime.new_object(None).unwrap();
        let queued_id = queued.object_id();
        {
            let _state = runtime.0.state.borrow();
            drop(queued);
        }
        // The right scalar cannot supply the old left-owner release boundary.
        let result = add_primitives(&runtime, left, JsValue::Int(1)).unwrap();
        let reused = matches!(result, JsValue::String(id) if id == original);
        let text = finish(&runtime, result);
        assert!(!reused);
        assert_eq!(text, JsString::from_static("a1"));
        assert!(runtime.0.state.borrow().heap.object(queued_id).is_err());
        assert!(!runtime.0.deferred_references.has_pending());
    }
}
