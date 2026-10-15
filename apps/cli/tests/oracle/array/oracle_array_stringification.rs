use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// This target pins QuickJS 2026-06-04's `js_array_join` and
// `js_array_toString`, including UTF-16 assembly, locale dispatch, recursive
// array stringification, and the intrinsic Object.prototype.toString fallback.

#[test]
fn array_stringification_boxing_errors_user_throws_and_overflow_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
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
    let caller_internal_error = eval_object(
        &mut caller,
        "InternalError.prototype",
        "caller InternalError prototype",
    );
    let join = property_callable(&runtime, &mut defining, &defining_array_prototype, "join");

    drop(
        defining
            .eval("Number.prototype.length=1;Number.prototype[0]='defining-box'")
            .expect("install defining realm Number array-like properties"),
    );
    drop(
        caller
            .eval("Number.prototype.length=1;Number.prototype[0]='caller-box'")
            .expect("install caller realm Number array-like properties"),
    );
    assert_eq!(
        caller
            .call(&join, Value::Int(7), &[])
            .expect("cross-realm primitive Array.join"),
        Value::String(JsString::try_from_utf8("defining-box").unwrap()),
        "Array.join boxed a primitive receiver outside its defining realm",
    );

    let bad_length = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=Symbol('length');return source})()",
        "caller object with Symbol length",
    );
    assert!(matches!(
        caller.call(&join, Value::Object(bad_length), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "cross-realm Array.join TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.join native TypeError did not use the method defining realm",
    );

    let throwing_length = eval_object(
        &mut caller,
        "(function(){var source=Object();source.__defineGetter__('length',function(){throw new TypeError('caller length')});return source})()",
        "caller object with throwing length getter",
    );
    assert!(matches!(
        caller.call(&join, Value::Object(throwing_length), &[]),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "cross-realm Array.join user throw");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.join replaced a user getter throw with a defining-realm error",
    );

    let cycle = eval_object(
        &mut caller,
        "(function(){var source=[];source[0]=source;return source})()",
        "caller recursive Array",
    );
    assert!(matches!(
        caller.call(&join, Value::Object(cycle), &[]),
        Err(RuntimeError::Exception),
    ));
    let overflow = take_exception_object(&mut caller, "cross-realm Array.join overflow");
    assert_eq!(
        runtime.get_prototype_of(&overflow).unwrap(),
        Some(caller_internal_error),
        "recursive Array.join did not allocate overflow in the recursively invoked method realm",
    );
}
