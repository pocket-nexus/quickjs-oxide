use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// This target pins QuickJS 2026-06-04's shared `js_array_pop` and
// `js_array_push` kernels. It covers both magic selectors for each kernel:
// pop/shift and push/unshift.

#[test]
fn array_mutator_boxing_native_errors_and_user_throws_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let defining_number_prototype = eval_object(
        &mut defining,
        "Number.prototype",
        "defining Number prototype",
    );
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
    let pop = property_callable(&runtime, &mut defining, &defining_array_prototype, "pop");
    let push = property_callable(&runtime, &mut defining, &defining_array_prototype, "push");

    drop(defining
        .eval(
            "Number.prototype.length=0;Number.prototype.__defineSetter__('0',function(value){globalThis.mutatorBox=this;globalThis.mutatorValue=value})",
        )
        .expect("install defining realm primitive push observers"));
    assert_eq!(
        caller
            .call(
                &push,
                Value::Int(7),
                &[Value::String(JsString::try_from_utf8("x").unwrap())],
            )
            .expect("cross-realm primitive Array.push"),
        Value::Int(1),
    );
    let boxed = eval_object(&mut defining, "mutatorBox", "captured primitive push box");
    assert_eq!(
        runtime.get_prototype_of(&boxed).unwrap(),
        Some(defining_number_prototype),
        "Array.push boxed its primitive receiver outside the defining realm",
    );
    assert_eq!(
        defining.eval("mutatorValue").unwrap(),
        Value::String(JsString::try_from_utf8("x").unwrap()),
        "Array.push changed an argument while crossing realms",
    );

    let too_long = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=9007199254740991;return source})()",
        "caller MAX_SAFE array-like",
    );
    assert!(matches!(
        caller.call(&push, Value::Object(too_long), &[Value::Int(1)]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.push MAX_SAFE TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "Array.push native TypeError did not use the method defining realm",
    );

    let fixed_tail = eval_object(
        &mut caller,
        "(function(){var source=Object(),descriptor=Object();source.length=1;descriptor.value='tail';descriptor.writable=true;descriptor.enumerable=true;descriptor.configurable=false;Object.defineProperty(source,'0',descriptor);return source})()",
        "caller non-configurable pop tail",
    );
    assert!(matches!(
        caller.call(&pop, Value::Object(fixed_tail), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.pop Delete TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.pop native TypeError did not use the method defining realm",
    );

    let throwing_length = eval_object(
        &mut caller,
        "(function(){var source=Object();source.__defineGetter__('length',function(){throw new TypeError('caller length')});return source})()",
        "caller throwing mutator length",
    );
    assert!(matches!(
        caller.call(&push, Value::Object(throwing_length), &[Value::Int(1)]),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.push length getter TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.push replaced a user getter throw with a defining-realm error",
    );
}
