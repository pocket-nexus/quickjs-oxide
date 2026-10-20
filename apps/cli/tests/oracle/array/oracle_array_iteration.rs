use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::core::{JsString, Value};

// This target pins the non-allocating modes of QuickJS 2026-06-04's shared
// `js_array_every` kernel: every, some, and forEach.

#[test]
fn array_iteration_boxing_native_errors_and_user_throws_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let defining_string_prototype = defining.string_prototype().unwrap();
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let every = property_callable(&runtime, &mut defining, &defining_array_prototype, "every");

    let capture = eval_callable(
        &runtime,
        &mut caller,
        "(function(value,index,receiver){globalThis.iterationReceiver=receiver;return true})",
        "caller receiver-capturing callback",
    );
    assert_eq!(
        caller
            .call(
                &every,
                Value::String(JsString::try_from_utf8("a").unwrap()),
                &[Value::Object(
                    capture.as_object().try_clone().expect("duplicate root")
                )],
            )
            .expect("cross-realm primitive Array.every call"),
        Value::Bool(true),
    );
    let captured = eval_object(
        &mut caller,
        "iterationReceiver",
        "captured iteration receiver",
    );
    assert_eq!(
        runtime.get_prototype_of(&captured).unwrap(),
        Some(defining_string_prototype),
        "Array iteration boxed its callback receiver in the caller realm",
    );

    let empty = eval_object(&mut caller, "[]", "caller empty Array");
    let native_error = exception_object(
        caller.call(&every, Value::Object(empty), &[Value::Int(0)]),
        "Array.every callback TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.every native TypeError did not use the method defining realm",
    );

    let throwing = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw new TypeError('caller callback')})",
        "caller throwing iteration callback",
    );
    let one = eval_object(&mut caller, "[1]", "caller one-element Array");
    let user_error = exception_object(
        caller.call(
            &every,
            Value::Object(one),
            &[Value::Object(
                throwing.as_object().try_clone().expect("duplicate root"),
            )],
        ),
        "Array.every user callback error",
    );
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.every replaced a user callback throw with a defining-realm error",
    );
}
