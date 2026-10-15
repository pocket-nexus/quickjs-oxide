use super::support::*;
use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::int_property;
use quickjs_oxide::engine::api::{RuntimeError, Value};

// This target pins QuickJS 2026-06-04 `Array.prototype.reverse` and
// `Array.prototype.toReversed`. The former mutates generic array-like receivers
// pair by pair; the latter is a dense defining-realm base-Array copy and never
// consults constructor or @@species.

#[test]
fn array_reverse_boxing_results_native_errors_and_user_throws_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let caller_array_prototype = caller.array_prototype().unwrap();
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
    let defining_range_error = eval_object(
        &mut defining,
        "RangeError.prototype",
        "defining RangeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let reverse = property_callable(
        &runtime,
        &mut defining,
        &defining_array_prototype,
        "reverse",
    );
    let to_reversed = property_callable(
        &runtime,
        &mut defining,
        &defining_array_prototype,
        "toReversed",
    );

    let Value::Object(boxed) = caller
        .call(&reverse, Value::Int(7), &[])
        .expect("cross-realm primitive Array.reverse")
    else {
        panic!("cross-realm primitive Array.reverse did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&boxed).unwrap(),
        Some(defining_number_prototype),
        "Array.reverse boxed its primitive receiver outside the defining realm",
    );

    let receiver = eval_object(&mut caller, "[1,2,3]", "caller Array receiver");
    let Value::Object(result) = caller
        .call(&to_reversed, Value::Object(receiver), &[])
        .expect("cross-realm Array.toReversed")
    else {
        panic!("cross-realm Array.toReversed did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "Array.toReversed result did not use the native defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype),
    );
    assert_eq!(int_property(&runtime, &mut caller, &result, "0"), 3);
    assert_eq!(int_property(&runtime, &mut caller, &result, "2"), 1);

    let too_long = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=2147483648;return source})()",
        "caller oversized array-like",
    );
    assert!(matches!(
        caller.call(&to_reversed, Value::Object(too_long), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.toReversed RangeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_range_error),
        "Array.toReversed RangeError did not use the method defining realm",
    );

    let fixed_upper = eval_object(
        &mut caller,
        "(function(){var source=Object(),descriptor=Object();source.length=2;descriptor.value='upper';descriptor.writable=true;descriptor.enumerable=true;descriptor.configurable=false;Object.defineProperty(source,'1',descriptor);return source})()",
        "caller non-configurable reverse upper",
    );
    assert!(matches!(
        caller.call(&reverse, Value::Object(fixed_upper), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.reverse Delete TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.reverse native TypeError did not use the method defining realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=2;source.__defineGetter__('0',function(){throw new TypeError('caller getter')});return source})()",
        "caller throwing reverse receiver",
    );
    assert!(matches!(
        caller.call(&reverse, Value::Object(throwing_receiver), &[]),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.reverse user getter TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error.try_clone().expect("duplicate root")),
        "Array.reverse replaced a user getter throw with a defining-realm error",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=2;source.__defineGetter__('1',function(){throw new TypeError('caller copy getter')});return source})()",
        "caller throwing toReversed receiver",
    );
    assert!(matches!(
        caller.call(&to_reversed, Value::Object(throwing_receiver), &[]),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.toReversed user getter TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.toReversed replaced a user getter throw with a defining-realm error",
    );
}
