use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_exception_object;

use crate::object_graph_observation::{
    define_global_with_message as define_global, eval_string_with_global,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

#[test]
fn typed_array_of_cross_realm_results_errors_and_abrupt_values() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let mut custom = runtime.new_context().expect("create context");

    let of = eval_callable(
        &runtime,
        &mut defining,
        "Object.getPrototypeOf(Uint8Array).of",
        "defining TypedArray.of",
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
    let custom_uint16_prototype = eval_object(
        &mut custom,
        "Uint16Array.prototype",
        "custom Uint16Array prototype",
    );
    let constructor = eval_callable(
        &runtime,
        &mut custom,
        "(function Result(length){return new Uint16Array(length)})",
        "custom result constructor",
    );

    let result = caller
        .call(
            &of,
            Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
            &[Value::number(41.0), Value::number(65578.0)],
        )
        .expect("cross-realm TypedArray.of");
    let Value::Object(result) = result else {
        panic!("cross-realm TypedArray.of did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(custom_uint16_prototype),
        "custom constructor result escaped its defining realm",
    );
    assert_eq!(
        eval_string_with_global(
            &runtime,
            &mut caller,
            "__typedArrayOfResult",
            Value::Object(result),
            "[__typedArrayOfResult.length,__typedArrayOfResult[0],\
             __typedArrayOfResult[1]].join('|')",
        ),
        "2|41|42",
    );

    let ordinary = eval_object(&mut caller, "({})", "caller ordinary receiver");
    assert!(matches!(
        caller.call(&of, Value::Object(ordinary), &[]),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm receiver TypeError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "receiver TypeError did not use the builtin defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error.try_clone().expect("duplicate root")),
        "receiver TypeError leaked into the caller realm",
    );

    assert!(matches!(
        caller.call(&of, Value::number(1.0), &[]),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm primitive receiver TypeError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "primitive receiver TypeError did not use the builtin defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error),
        "primitive receiver TypeError leaked into the caller realm",
    );

    let token = eval_object(&mut caller, "({token:1})", "caller abrupt token");
    let item = eval_object(
        &mut caller,
        "({valueOf:function(){throw __typedArrayOfToken}})",
        "caller abrupt item",
    );
    define_global(
        &runtime,
        &mut caller,
        "__typedArrayOfToken",
        Value::Object(token.try_clone().expect("duplicate root")),
    );
    assert!(matches!(
        caller.call(
            &of,
            Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
            &[Value::Object(item)],
        ),
        Err(RuntimeError::Exception),
    ));
    let thrown = caller
        .take_exception()
        .expect("take cross-realm abrupt value")
        .expect("cross-realm abrupt value was missing");
    assert_eq!(thrown, Value::Object(token));

    let symbol = caller
        .eval("Symbol('typed-array-of')")
        .expect("caller symbol value");
    assert!(matches!(
        caller.call(
            &of,
            Value::Object(constructor.as_object().try_clone().expect("duplicate root")),
            &[symbol],
        ),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm conversion TypeError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error),
        "element conversion TypeError did not use the builtin defining realm",
    );
}
