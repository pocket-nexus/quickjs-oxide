use crate::object_graph_observation::{global_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;

use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::core::{RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_object_is`. The upstream builtin is a direct
// `js_same_value` call: it never boxes or coerces either argument and it has no
// object-family-specific behavior to defer to later Proxy/TypedArray slices.

#[test]
fn object_is_cross_realm_identity_and_constructor_error_realm_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let object_is = property_callable(&runtime, &mut defining, defining_object.as_object(), "is");
    let source = caller.new_object().unwrap();
    let other = caller.new_object().unwrap();

    assert_eq!(
        caller
            .call(
                &object_is,
                Value::Object(other.try_clone().expect("duplicate root")),
                &[
                    Value::Object(source.try_clone().expect("duplicate root")),
                    Value::Object(source.try_clone().expect("duplicate root"))
                ],
            )
            .unwrap(),
        Value::Bool(true),
    );
    assert_eq!(
        caller
            .call(
                &object_is,
                Value::Undefined,
                &[Value::Object(source), Value::Object(other)],
            )
            .unwrap(),
        Value::Bool(false),
    );
    assert_eq!(
        caller
            .call(
                &object_is,
                Value::Undefined,
                &[Value::Int(0), Value::Float(-0.0)],
            )
            .unwrap(),
        Value::Bool(false),
    );
    assert_eq!(
        caller
            .call(
                &object_is,
                Value::Undefined,
                &[Value::Float(f64::NAN), Value::Float(f64::NAN)],
            )
            .unwrap(),
        Value::Bool(true),
    );

    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_eq!(
        caller.construct(&object_is, &[]),
        Err(RuntimeError::Exception)
    );
    let error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error),
        "non-constructor rejection must use the caller realm",
    );
}
