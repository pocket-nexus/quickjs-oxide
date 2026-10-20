use crate::object_graph_observation::{eval_object, global_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;

use crate::runtime_observation::{pending_exception_object as exception_object, property_callable};
use quickjs_oxide::engine::api::core::Value;

// Pins QuickJS 2026-06-04 `js_object_seal` and `js_object_isSealed` for
// Object.seal/freeze/isSealed/isFrozen. Proxy trap invariants and non-empty
// TypedArray freeze failures participate in this differential. Module
// namespace objects remain a separate module/object-model boundary; mapped and
// unmapped Arguments integrity semantics are locked by their dedicated
// differential.

#[test]
fn object_integrity_cross_realm_identity_and_constructor_error_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let seal = property_callable(&runtime, &mut defining, defining_object.as_object(), "seal");
    let freeze = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "freeze",
    );
    let is_sealed = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "isSealed",
    );
    let is_frozen = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "isFrozen",
    );

    let object = eval_object(
        &mut caller,
        "(function(){var value=Object();value.x=1;return value})()",
    );
    assert_eq!(
        caller
            .call(
                &seal,
                Value::Undefined,
                &[Value::Object(object.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        Value::Object(object.try_clone().expect("duplicate root")),
    );
    assert_eq!(
        caller
            .call(
                &is_sealed,
                Value::Undefined,
                &[Value::Object(object.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        Value::Bool(true),
    );
    assert_eq!(
        caller
            .call(
                &is_frozen,
                Value::Undefined,
                &[Value::Object(object.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        Value::Bool(false),
    );
    assert_eq!(
        caller
            .call(
                &freeze,
                Value::Undefined,
                &[Value::Object(object.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        Value::Object(object.try_clone().expect("duplicate root")),
    );
    assert_eq!(
        caller
            .call(&is_frozen, Value::Undefined, &[Value::Object(object)])
            .unwrap(),
        Value::Bool(true),
    );
    assert_eq!(
        caller
            .call(&freeze, Value::Null, &[Value::Float(-0.0)])
            .unwrap(),
        Value::Float(-0.0),
    );

    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    let error = exception_object(caller.construct(&seal, &[]));
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error),
        "non-constructor rejection must use the caller realm",
    );
}
