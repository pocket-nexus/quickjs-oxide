use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// This target pins QuickJS 2026-06-04's `js_array_concat`, including
// `JS_ArraySpeciesCreate`, `@@isConcatSpreadable`, holes, and final length Set.

#[test]
fn array_concat_species_boxing_results_and_errors_use_pinned_realms() {
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
    let caller_object_prototype = caller.object_prototype().unwrap();
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let concat = property_callable(&runtime, &mut defining, &defining_array_prototype, "concat");

    let source = eval_object(&mut caller, "[1]", "caller Array");
    let Value::Object(result) = caller
        .call(&concat, Value::Object(source), &[])
        .expect("cross-realm default Array.concat")
    else {
        panic!("cross-realm Array.concat did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "cross-realm default Array constructor was not replaced by the method realm Array",
    );

    let Value::Object(string_result) = caller
        .call(
            &concat,
            Value::String(JsString::try_from_utf8("a").unwrap()),
            &[],
        )
        .expect("cross-realm primitive Array.concat")
    else {
        panic!("primitive Array.concat did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&string_result).unwrap(),
        Some(defining_array_prototype),
        "primitive Array.concat result did not use the method realm Array",
    );
    let zero = runtime.intern_property_key("0").unwrap();
    let Value::Object(boxed) = caller.get_property(&string_result, &zero).unwrap() else {
        panic!("primitive Array.concat did not store its boxed receiver");
    };
    assert_eq!(
        runtime.get_prototype_of(&boxed).unwrap(),
        Some(defining_string_prototype),
        "Array.concat boxed its primitive receiver outside the defining realm",
    );

    let custom_source = eval_object(
        &mut caller,
        "(function(){var source=[1],ctor=Object();ctor[Symbol.species]=function(length){globalThis.concatSpeciesLength=length;return Object()};source.constructor=ctor;return source})()",
        "caller Array with custom concat species",
    );
    let Value::Object(custom_result) = caller
        .call(&concat, Value::Object(custom_source), &[])
        .expect("cross-realm custom species Array.concat")
    else {
        panic!("custom species Array.concat did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&custom_result).unwrap(),
        Some(caller_object_prototype.try_clone().expect("duplicate root")),
        "Array.concat moved a custom species result into the method realm",
    );
    assert_eq!(
        caller.eval("concatSpeciesLength").unwrap(),
        Value::Int(0),
        "Array.concat passed a nonzero length to custom species",
    );

    drop(
        caller
            .eval("globalThis.concatArgument=Object()")
            .expect("install caller concat argument"),
    );
    let argument = eval_object(&mut caller, "concatArgument", "caller concat argument");
    let empty = eval_object(&mut caller, "[]", "caller empty Array");
    let Value::Object(identity_result) = caller
        .call(
            &concat,
            Value::Object(empty.try_clone().expect("duplicate root")),
            &[Value::Object(argument.try_clone().expect("duplicate root"))],
        )
        .expect("cross-realm object argument concat")
    else {
        panic!("object argument concat did not return an object");
    };
    let Value::Object(copied) = caller.get_property(&identity_result, &zero).unwrap() else {
        panic!("Array.concat did not retain its object argument");
    };
    assert_eq!(
        copied, argument,
        "Array.concat cloned or moved an object argument"
    );

    let bad = eval_object(
        &mut caller,
        "(function(){var source=[1];source.constructor=1;return source})()",
        "caller Array with primitive concat constructor",
    );
    assert!(matches!(
        caller.call(&concat, Value::Object(bad), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.concat species TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.concat native TypeError did not use the method defining realm",
    );

    let throwing_argument = eval_object(
        &mut caller,
        "(function(){var value=Object();value.__defineGetter__(Symbol.isConcatSpreadable,function(){throw new TypeError('caller spread getter')});return value})()",
        "caller throwing spreadability argument",
    );
    assert!(matches!(
        caller.call(
            &concat,
            Value::Object(empty),
            &[Value::Object(throwing_argument)],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.concat spread getter TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.concat replaced a user getter throw with a defining-realm error",
    );
}
