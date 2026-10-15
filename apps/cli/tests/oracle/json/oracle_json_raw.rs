use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

#[test]
fn json_raw_brand_and_native_errors_cross_realms() {
    // The qjs CLI has no same-runtime multi-context bridge. Exercise the
    // runtime-wide brand directly and pin native SyntaxError allocation to the
    // intrinsic method's defining realm.
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_syntax_error = eval_object(
        &mut defining,
        "SyntaxError.prototype",
        "defining SyntaxError prototype",
    );
    let caller_syntax_error = eval_object(
        &mut caller,
        "SyntaxError.prototype",
        "caller SyntaxError prototype",
    );
    let defining_json = eval_object(&mut defining, "JSON", "defining JSON object");
    let caller_json = eval_object(&mut caller, "JSON", "caller JSON object");
    let raw_json = property_callable(&runtime, &mut defining, &defining_json, "rawJSON");
    let defining_is_raw = property_callable(&runtime, &mut defining, &defining_json, "isRawJSON");
    let caller_is_raw = property_callable(&runtime, &mut caller, &caller_json, "isRawJSON");

    let Value::Object(raw) = caller
        .call(
            &raw_json,
            Value::Undefined,
            &[Value::String(JsString::try_from_utf8("42").unwrap())],
        )
        .expect("cross-realm JSON.rawJSON call")
    else {
        panic!("cross-realm JSON.rawJSON result was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&raw).unwrap(),
        None,
        "RawJSON object did not retain its null prototype",
    );
    for (description, context, predicate) in [
        ("defining", &mut defining, &defining_is_raw),
        ("caller", &mut caller, &caller_is_raw),
    ] {
        assert_eq!(
            context
                .call(
                    predicate,
                    Value::Undefined,
                    &[Value::Object(raw.try_clone().expect("duplicate root"))],
                )
                .unwrap_or_else(|error| panic!("{description} isRawJSON call: {error}")),
            Value::Bool(true),
            "{description} realm did not recognize the shared RawJSON brand",
        );
    }

    let forged = eval_object(
        &mut caller,
        r#"
            (function () {
                var value = Object.create(null);
                Object.defineProperty(value, "rawJSON", {
                    value: "42", writable: false, enumerable: true, configurable: false
                });
                return Object.preventExtensions(value);
            })()
        "#,
        "caller forged RawJSON object",
    );
    assert_eq!(
        caller
            .call(&defining_is_raw, Value::Undefined, &[Value::Object(forged)],)
            .expect("cross-realm forged brand check"),
        Value::Bool(false),
    );

    assert!(matches!(
        caller.call(
            &raw_json,
            Value::Undefined,
            &[Value::String(JsString::try_from_utf8("[]").unwrap())],
        ),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm JSON.rawJSON SyntaxError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_syntax_error),
        "JSON.rawJSON allocated SyntaxError in the caller realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_syntax_error),
        "JSON.rawJSON native SyntaxError unexpectedly used the caller realm",
    );
}
