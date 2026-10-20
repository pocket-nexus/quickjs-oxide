use crate::object_graph_observation::{eval_object, global_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;

use crate::runtime_observation::{pending_exception_object as exception_object, property_callable};
use quickjs_oxide::engine::api::core::{Context, ObjectRef, Runtime, Value};

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
    let framework_error = exception_object(caller.call(&keys, Value::Undefined, &[Value::Null]));
    assert_eq!(
        runtime.get_prototype_of(&framework_error).unwrap(),
        Some(defining_type_error)
    );

    let caller_range_error = intrinsic_prototype(&runtime, &mut caller, "RangeError");
    let throwing = eval_object(
        &mut caller,
        r#"(function(){var value=Object();value.__defineGetter__("x",function(){throw new RangeError("user")});return value})()"#,
    );
    let user_error =
        exception_object(caller.call(&values, Value::Undefined, &[Value::Object(throwing)]));
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_range_error)
    );
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
