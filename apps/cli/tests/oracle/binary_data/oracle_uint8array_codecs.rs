use crate::object_graph_observation::int_property;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{RuntimeError, Value};

#[test]
fn uint8array_codec_results_and_errors_use_the_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let defining_constructor = eval_object(&mut defining, "Uint8Array", "defining Uint8Array");
    let defining_prototype = eval_object(
        &mut defining,
        "Uint8Array.prototype",
        "defining Uint8Array prototype",
    );
    let defining_object_prototype = eval_object(
        &mut defining,
        "Object.prototype",
        "defining Object prototype",
    );
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let defining_syntax_error = eval_object(
        &mut defining,
        "SyntaxError.prototype",
        "defining SyntaxError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );

    let from_base64 =
        property_callable(&runtime, &mut defining, &defining_constructor, "fromBase64");
    let to_base64 = property_callable(&runtime, &mut defining, &defining_prototype, "toBase64");
    let to_hex = property_callable(&runtime, &mut defining, &defining_prototype, "toHex");
    let set_from_base64 = property_callable(
        &runtime,
        &mut defining,
        &defining_prototype,
        "setFromBase64",
    );

    let input = caller.eval("'AQI='").expect("caller base64 input");
    let Value::Object(decoded) = caller
        .call(&from_base64, Value::Undefined, &[input])
        .expect("cross-realm Uint8Array.fromBase64")
    else {
        panic!("cross-realm Uint8Array.fromBase64 was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&decoded).unwrap(),
        Some(defining_prototype.try_clone().expect("duplicate root")),
        "Uint8Array.fromBase64 result did not use the method defining realm",
    );
    assert_eq!(int_property(&runtime, &mut caller, &decoded, "0"), 1);
    assert_eq!(int_property(&runtime, &mut caller, &decoded, "1"), 2);

    let receiver = eval_object(
        &mut caller,
        "new Uint8Array([10,11])",
        "caller Uint8Array receiver",
    );
    let Value::String(hex) = caller
        .call(
            &to_hex,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[],
        )
        .expect("cross-realm Uint8Array.prototype.toHex")
    else {
        panic!("cross-realm Uint8Array.prototype.toHex was not a string");
    };
    assert_eq!(hex.to_utf8_lossy(), "0a0b");

    let set_input = caller.eval("'DA0='").expect("caller set input");
    let Value::Object(record) = caller
        .call(
            &set_from_base64,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[set_input],
        )
        .expect("cross-realm Uint8Array.prototype.setFromBase64")
    else {
        panic!("cross-realm setFromBase64 record was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&record).unwrap(),
        Some(defining_object_prototype),
        "setFromBase64 record did not use the method defining realm",
    );
    assert_eq!(int_property(&runtime, &mut caller, &record, "read"), 4);
    assert_eq!(int_property(&runtime, &mut caller, &record, "written"), 2);
    assert_eq!(int_property(&runtime, &mut caller, &receiver, "0"), 12);
    assert_eq!(int_property(&runtime, &mut caller, &receiver, "1"), 13);

    let invalid = caller.eval("'!'").expect("caller invalid base64");
    assert!(matches!(
        caller.call(&from_base64, Value::Undefined, &[invalid]),
        Err(RuntimeError::Exception),
    ));
    let syntax_error = take_exception_object(&mut caller, "Uint8Array.fromBase64 SyntaxError");
    assert_eq!(
        runtime.get_prototype_of(&syntax_error).unwrap(),
        Some(defining_syntax_error),
        "Uint8Array.fromBase64 SyntaxError did not use the defining realm",
    );

    let wrong_receiver = eval_object(&mut caller, "Object()", "caller wrong receiver");
    assert!(matches!(
        caller.call(&to_hex, Value::Object(wrong_receiver), &[]),
        Err(RuntimeError::Exception),
    ));
    let type_error = take_exception_object(&mut caller, "Uint8Array.prototype.toHex TypeError");
    assert_eq!(
        runtime.get_prototype_of(&type_error).unwrap(),
        Some(defining_type_error),
        "Uint8Array.prototype.toHex TypeError did not use the defining realm",
    );

    let throwing_options = eval_object(
        &mut caller,
        "({get alphabet(){throw new TypeError('caller getter')}})",
        "caller throwing codec options",
    );
    assert!(matches!(
        caller.call(
            &to_base64,
            Value::Object(receiver),
            &[Value::Object(throwing_options)],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Uint8Array codec user getter error");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Uint8Array codec replaced a caller getter throw",
    );
}
