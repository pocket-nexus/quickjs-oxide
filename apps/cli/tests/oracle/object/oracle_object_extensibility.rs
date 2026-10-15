use crate::object_graph_observation::{global_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;

use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_object_isExtensible` and
// `js_object_preventExtensions`. Proxy [[IsExtensible]]/[[PreventExtensions]]
// traps and Resizable ArrayBuffer-backed TypedArrays participate in the same
// differential.

#[test]
fn object_extensibility_cross_realm_calls_and_constructor_error_realm_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let is_extensible = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "isExtensible",
    );
    let prevent_extensions = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "preventExtensions",
    );
    let source = caller.new_object().unwrap();

    assert_eq!(
        caller
            .call(
                &is_extensible,
                Value::Undefined,
                &[Value::Object(source.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        Value::Bool(true),
    );
    assert_eq!(
        caller
            .call(
                &prevent_extensions,
                Value::Undefined,
                &[Value::Object(source.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        Value::Object(source.try_clone().expect("duplicate root")),
    );
    assert_eq!(
        caller
            .call(&is_extensible, Value::Undefined, &[Value::Object(source)],)
            .unwrap(),
        Value::Bool(false),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    for method in [&is_extensible, &prevent_extensions] {
        assert_eq!(caller.construct(method, &[]), Err(RuntimeError::Exception));
        let error = take_exception_object(&mut caller);
        assert_eq!(
            runtime.get_prototype_of(&error).unwrap(),
            Some(caller_type_error.try_clone().expect("duplicate root")),
            "non-constructor rejection must use the caller realm",
        );
    }
}

#[test]
fn object_extensibility_methods_are_per_realm_and_retain_then_release_their_realm() {
    let runtime = host_runtime();
    let (is_extensible, prevent_extensions) = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_object = global_callable(&runtime, &mut first, "Object");
        let second_object = global_callable(&runtime, &mut second, "Object");
        let first_is = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "isExtensible",
        );
        let first_is_again = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "isExtensible",
        );
        let first_prevent = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "preventExtensions",
        );
        let second_is = property_callable(
            &runtime,
            &mut second,
            second_object.as_object(),
            "isExtensible",
        );
        assert_eq!(first_is, first_is_again);
        assert_ne!(first_is, first_prevent);
        assert_ne!(first_is, second_is);
        assert_eq!(
            runtime.get_prototype_of(first_is.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        drop(second_is);
        (first_is, first_prevent)
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(is_extensible);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(prevent_extensions);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}
