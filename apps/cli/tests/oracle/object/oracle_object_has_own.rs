use crate::object_graph_observation::{eval_object, global_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;

use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_object_hasOwn`. Unlike the legacy
// `Object.prototype.hasOwnProperty`, the static first performs ToObject on its
// first argument and only then ToPropertyKey on its second argument. The final
// query is an own-property descriptor probe and must not Get the property.
// Proxy, TypedArray, and Arguments exotic own-property paths participate in
// this differential. Module namespace objects remain a separate module
// boundary.

#[test]
fn object_has_own_cross_realm_objects_errors_and_user_throws_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let has_own = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "hasOwn",
    );

    let prototype = caller.new_object().unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    assert!(
        caller
            .set_property(&prototype, &key, Value::Int(1))
            .unwrap()
    );
    let inherited = caller.new_object_with_prototype(Some(&prototype)).unwrap();
    let own = caller.new_object().unwrap();
    assert!(caller.set_property(&own, &key, Value::Int(2)).unwrap());
    let key_value = Value::String(JsString::try_from_utf8("x").unwrap());
    assert_eq!(
        caller
            .call(
                &has_own,
                Value::Object(prototype),
                &[
                    Value::Object(own),
                    key_value.try_clone().expect("duplicate root")
                ],
            )
            .unwrap(),
        Value::Bool(true),
    );
    assert_eq!(
        caller
            .call(
                &has_own,
                Value::Undefined,
                &[Value::Object(inherited), key_value],
            )
            .unwrap(),
        Value::Bool(false),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_eq!(
        caller.call(
            &has_own,
            Value::Undefined,
            &[
                Value::Null,
                Value::String(JsString::try_from_utf8("x").unwrap()),
            ],
        ),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "native ToObject rejection did not use the defining realm",
    );

    let sentinel = caller.new_object().unwrap();
    let sentinel_key = runtime.intern_property_key("hasOwnSentinel").unwrap();
    assert!(
        caller
            .set_property(
                &caller.global_object().unwrap(),
                &sentinel_key,
                Value::Object(sentinel.try_clone().expect("duplicate root")),
            )
            .unwrap()
    );
    let throwing_key = eval_object(
        &mut caller,
        r#"(function(){
            var key=Object();
            key[Symbol.toPrimitive]=function(){throw hasOwnSentinel};
            return key;
        })()"#,
    );
    let target = caller.new_object().unwrap();
    assert_eq!(
        caller.call(
            &has_own,
            Value::Undefined,
            &[Value::Object(target), Value::Object(throwing_key)],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "property-key conversion throw identity was not preserved",
    );

    assert_eq!(
        caller.construct(&has_own, &[]),
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
fn object_has_own_method_is_per_realm_and_retain_then_releases_its_realm() {
    let runtime = host_runtime();
    let has_own = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_object = global_callable(&runtime, &mut first, "Object");
        let second_object = global_callable(&runtime, &mut second, "Object");
        let first_method =
            property_callable(&runtime, &mut first, first_object.as_object(), "hasOwn");
        let first_method_again =
            property_callable(&runtime, &mut first, first_object.as_object(), "hasOwn");
        let second_method =
            property_callable(&runtime, &mut second, second_object.as_object(), "hasOwn");
        assert_eq!(first_method, first_method_again);
        assert_ne!(first_method, second_method);
        assert_eq!(
            runtime.get_prototype_of(first_method.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        drop(second_method);
        first_method
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(has_own);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}
