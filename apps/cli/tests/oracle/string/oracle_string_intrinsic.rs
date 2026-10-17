use crate::object_graph_observation::{eval_object, intrinsic_prototype};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::core::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_string_constructor`, `js_string_fromCharCode`,
// `js_string_fromCodePoint`, `js_string_raw`, and `js_string_funcs`. The
// constructor-owned table is treated as one milestone because QuickJS installs
// its three lazy statics before the non-configurable `prototype` property. If
// `%String%` were published first and the statics appended later, observable
// own-key order could never be repaired.

#[test]
fn string_cross_realm_results_errors_and_user_throws_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_global = defining.global_object().unwrap();
    let string = property_callable(&runtime, &mut defining, &defining_global, "String");
    let from_code_point =
        property_callable(&runtime, &mut defining, string.as_object(), "fromCodePoint");
    let raw = property_callable(&runtime, &mut defining, string.as_object(), "raw");

    let Value::Object(wrapper) = caller
        .construct(
            &string,
            &[Value::String(JsString::try_from_utf8("foreign").unwrap())],
        )
        .unwrap()
    else {
        panic!("foreign new String did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&wrapper).unwrap(),
        Some(defining.string_prototype().unwrap()),
        "ordinary construction did not use the constructor's defining realm",
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let defining_range_error = intrinsic_prototype(&runtime, &mut defining, "RangeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    let symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("construct").unwrap()))
        .unwrap();
    assert_eq!(
        caller.construct(&string, &[Value::Symbol(symbol)]),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
    );
    assert_eq!(
        caller.call(&from_code_point, Value::Undefined, &[Value::Int(-1)]),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(defining_range_error),
    );
    assert_eq!(
        caller.call(&raw, Value::Undefined, &[Value::Null]),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(defining_type_error),
    );

    let sentinel = caller.new_object().unwrap();
    let sentinel_key = runtime.intern_property_key("stringSentinel").unwrap();
    assert!(
        caller
            .set_property(
                &caller.global_object().unwrap(),
                &sentinel_key,
                Value::Object(sentinel.try_clone().expect("duplicate root")),
            )
            .unwrap()
    );
    let throwing = eval_object(
        &mut caller,
        r#"(function(){var value=Object();value[Symbol.toPrimitive]=function(){throw stringSentinel};return value})()"#,
    );
    assert_eq!(
        defining.call(&string, Value::Undefined, &[Value::Object(throwing)]),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        defining.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "an explicit user throw did not retain identity",
    );

    assert_eq!(caller.construct(&raw, &[]), Err(RuntimeError::Exception));
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(caller_type_error),
        "non-constructor rejection did not use the caller realm",
    );
}

#[test]
fn string_wrapper_retains_then_releases_its_realm_graph() {
    let runtime = host_runtime();
    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let global = context.global_object().unwrap();
        let string = property_callable(&runtime, &mut context, &global, "String");
        let Value::Object(wrapper) = context
            .construct(
                &string,
                &[Value::String(JsString::try_from_utf8("retained").unwrap())],
            )
            .unwrap()
        else {
            panic!("new String did not return a wrapper");
        };
        wrapper
    };
    runtime.run_gc().unwrap();
    drop(wrapper);
    runtime.run_gc().unwrap();
}
