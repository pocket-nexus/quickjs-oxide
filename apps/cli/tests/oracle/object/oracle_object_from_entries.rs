use crate::object_graph_observation::{eval_object, global_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;

use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_object_fromEntries`. In particular, the
// implementation allocates a defining-realm ordinary result, acquires and
// caches `next`, reads entry[0] before entry[1], converts the key only after
// both reads, and routes every abrupt completion after iterator acquisition
// through `JS_IteratorClose(..., TRUE)`.
//
// Proxy and TypedArray entries participate in the same differential. Strong
// Map/Set iterators and generator IteratorClose are active as well. Module
// namespace entries remain a separate module/object-model boundary.

#[test]
fn object_from_entries_uses_defining_realm_and_preserves_user_throws() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let from_entries = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "fromEntries",
    );

    let payload = caller.new_object().unwrap();
    let pair = caller
        .new_array_from_values(vec![
            Value::String(JsString::try_from_utf8("value").unwrap()),
            Value::Object(payload.try_clone().expect("duplicate root")),
        ])
        .unwrap();
    let entries = caller
        .new_array_from_values(vec![Value::Object(pair)])
        .unwrap();
    let Value::Object(result) = caller
        .call(&from_entries, Value::Undefined, &[Value::Object(entries)])
        .unwrap()
    else {
        panic!("cross-realm Object.fromEntries result was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(defining.object_prototype().unwrap()),
        "result did not use Object.fromEntries's defining realm",
    );
    assert_eq!(
        caller
            .get_property(&result, &runtime.intern_property_key("value").unwrap())
            .unwrap(),
        Value::Object(payload),
        "entry value identity was not preserved",
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_eq!(
        caller.call(&from_entries, Value::Undefined, &[Value::Null]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "native iterable rejection did not use the defining realm",
    );

    let bad_entry = caller.new_array_from_values(vec![Value::Int(1)]).unwrap();
    assert_eq!(
        caller.call(&from_entries, Value::Undefined, &[Value::Object(bad_entry)],),
        Err(RuntimeError::Exception),
    );
    let native_entry_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&native_entry_error).unwrap(),
        Some(defining_type_error),
        "native entry rejection did not use the defining realm",
    );

    let sentinel = caller.new_object().unwrap();
    let sentinel_key = runtime.intern_property_key("fromEntriesSentinel").unwrap();
    assert!(
        caller
            .set_property(
                &caller.global_object().unwrap(),
                &sentinel_key,
                Value::Object(sentinel.try_clone().expect("duplicate root")),
            )
            .unwrap()
    );
    let throwing_pair = eval_object(
        &mut caller,
        r#"(function(){
            var pair=Object();
            pair.__defineGetter__("0",function(){throw fromEntriesSentinel});
            pair[1]=1;
            return pair;
        })()"#,
    );
    let throwing_entries = caller
        .new_array_from_values(vec![Value::Object(throwing_pair)])
        .unwrap();
    assert_eq!(
        caller.call(
            &from_entries,
            Value::Undefined,
            &[Value::Object(throwing_entries)],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "entry getter throw identity was not preserved",
    );

    assert_eq!(
        caller.construct(&from_entries, &[]),
        Err(RuntimeError::Exception),
    );
    let constructor_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&constructor_error).unwrap(),
        Some(caller_type_error),
        "non-constructor rejection did not use the caller realm",
    );
}

#[test]
fn object_from_entries_method_and_result_retain_then_release_their_realm() {
    let runtime = host_runtime();
    let (from_entries, result) = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_object = global_callable(&runtime, &mut first, "Object");
        let second_object = global_callable(&runtime, &mut second, "Object");
        let first_method = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "fromEntries",
        );
        let first_method_again = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "fromEntries",
        );
        let second_method = property_callable(
            &runtime,
            &mut second,
            second_object.as_object(),
            "fromEntries",
        );
        assert_eq!(first_method, first_method_again);
        assert_ne!(first_method, second_method);
        assert_eq!(
            runtime.get_prototype_of(first_method.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        drop(second_method);

        let empty = second.new_array().unwrap();
        let Value::Object(result) = second
            .call(&first_method, Value::Undefined, &[Value::Object(empty)])
            .unwrap()
        else {
            panic!("Object.fromEntries did not return an object");
        };
        assert_eq!(
            runtime.get_prototype_of(&result).unwrap(),
            Some(first.object_prototype().unwrap()),
        );
        (first_method, result)
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(from_entries);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(result);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}
