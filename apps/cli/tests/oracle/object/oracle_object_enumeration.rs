use crate::object_graph_observation::{eval_object, global_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;

use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{Context, ObjectRef, Runtime, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_object_keys` for Object.keys/values/entries.
// Proxy ownKeys/getOwnPropertyDescriptor/get traps are intentionally outside
// this milestone because the Rust runtime does not yet publish Proxy objects.

#[test]
fn object_enumeration_cross_realm_results_and_error_realms_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let keys = property_callable(&runtime, &mut defining, defining_object.as_object(), "keys");
    let values = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "values",
    );
    let entries = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "entries",
    );
    let defining_array = defining.array_prototype().unwrap();

    let source = eval_object(
        &mut caller,
        "(function(){var value=Object();value.x=1;return value})()",
    );
    let Value::Object(keys_result) = caller
        .call(
            &keys,
            Value::Undefined,
            &[Value::Object(source.try_clone().expect("duplicate root"))],
        )
        .unwrap()
    else {
        panic!("cross-realm Object.keys did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&keys_result).unwrap(),
        Some(defining_array.try_clone().expect("duplicate root"))
    );

    let Value::Object(entries_result) = caller
        .call(&entries, Value::Undefined, &[Value::Object(source)])
        .unwrap()
    else {
        panic!("cross-realm Object.entries did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&entries_result).unwrap(),
        Some(defining_array.try_clone().expect("duplicate root"))
    );
    let pair = object_index(&runtime, &mut caller, &entries_result, 0);
    assert_eq!(
        runtime.get_prototype_of(&pair).unwrap(),
        Some(defining_array)
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    assert_eq!(
        caller.call(&keys, Value::Undefined, &[Value::Null]),
        Err(RuntimeError::Exception),
    );
    let framework_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&framework_error).unwrap(),
        Some(defining_type_error)
    );

    let caller_range_error = intrinsic_prototype(&runtime, &mut caller, "RangeError");
    let throwing = eval_object(
        &mut caller,
        r#"(function(){var value=Object();value.__defineGetter__("x",function(){throw new RangeError("user")});return value})()"#,
    );
    assert_eq!(
        caller.call(&values, Value::Undefined, &[Value::Object(throwing)]),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_range_error)
    );
}

#[test]
fn object_enumeration_methods_are_per_realm_and_retain_then_release_their_realm() {
    let runtime = host_runtime();
    let (keys, values, entries) = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_object = global_callable(&runtime, &mut first, "Object");
        let second_object = global_callable(&runtime, &mut second, "Object");
        let first_keys = property_callable(&runtime, &mut first, first_object.as_object(), "keys");
        let first_keys_again =
            property_callable(&runtime, &mut first, first_object.as_object(), "keys");
        let first_values =
            property_callable(&runtime, &mut first, first_object.as_object(), "values");
        let first_entries =
            property_callable(&runtime, &mut first, first_object.as_object(), "entries");
        let second_keys =
            property_callable(&runtime, &mut second, second_object.as_object(), "keys");
        assert_eq!(first_keys, first_keys_again);
        assert_ne!(first_keys, first_values);
        assert_ne!(first_values, first_entries);
        assert_ne!(first_keys, second_keys);
        assert_eq!(
            runtime.get_prototype_of(first_keys.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        drop(second_keys);
        (first_keys, first_values, first_entries)
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(keys);
    drop(values);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(entries);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}

fn object_index(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    index: u32,
) -> ObjectRef {
    let Value::Object(value) = context
        .get_property(
            object,
            &runtime.intern_property_key(&index.to_string()).unwrap(),
        )
        .unwrap()
    else {
        panic!("index {index} was not an object");
    };
    value
}
