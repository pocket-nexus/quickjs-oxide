use crate::object_graph_observation::{
    callable_property, checked_property_value as object_property,
};
use crate::runtime_observation::exception_object;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::string_value;
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::core::Value;

// Differential lock for pinned QuickJS 2026-06-04
// `js_regexp_Symbol_matchAll` (`quickjs.c` 48419-48482) and the RegExp String
// Iterator next operation (`quickjs.c` 48365-48417).

#[test]
fn regexp_match_all_iterator_and_match_results_use_their_defining_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let method = eval_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype[Symbol.matchAll]",
        "defining RegExp Symbol.matchAll",
    );
    let defining_iterator_prototype = eval_object(
        &mut defining,
        "Object.getPrototypeOf(/x/g[Symbol.matchAll](\"\"))",
        "defining RegExp String Iterator prototype",
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
    let caller_array_prototype =
        eval_object(&mut caller, "Array.prototype", "caller Array prototype");
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );

    let foreign_regexp = eval_object(
        &mut caller,
        "(function(){var value=/a/g;value.lastIndex=1;return value})()",
        "caller RegExp",
    );
    let Value::Object(iterator) = caller
        .call(
            &method,
            Value::Object(foreign_regexp),
            &[string_value("ba")],
        )
        .expect("cross-realm RegExp matchAll")
    else {
        panic!("cross-realm RegExp matchAll did not return an iterator");
    };
    assert_eq!(
        runtime.get_prototype_of(&iterator).unwrap(),
        Some(defining_iterator_prototype),
        "RegExp String Iterator did not use the method defining realm",
    );

    let next = callable_property(&runtime, &mut caller, &iterator, "next");
    let Value::Object(iteration_result) = caller
        .call(
            &next,
            Value::Object(iterator.try_clone().expect("duplicate root")),
            &[],
        )
        .expect("cross-realm RegExp String Iterator next")
    else {
        panic!("RegExp String Iterator next did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&iteration_result).unwrap(),
        Some(defining_object_prototype),
        "iterator result wrapper did not use the iterator method realm",
    );
    let Value::Object(match_result) =
        object_property(&runtime, &mut caller, &iteration_result, "value")
    else {
        panic!("RegExp String Iterator did not yield a match object");
    };
    assert_eq!(
        runtime.get_prototype_of(&match_result).unwrap(),
        Some(caller_array_prototype),
        "species-created foreign RegExp did not retain its match-result Array realm",
    );

    let native_error = exception_object(
        caller.call(&method, Value::Int(1), &[string_value("x")]),
        "RegExp matchAll receiver TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "native RegExp matchAll TypeError did not use the method realm",
    );

    let throwing_input = eval_object(
        &mut caller,
        r#"(function(){
            var input=Object();
            input.toString=function(){throw new TypeError("caller input")};
            return input;
        })()"#,
        "caller throwing matchAll input",
    );
    let receiver = eval_object(&mut caller, "/a/g", "caller RegExp for throwing input");
    let user_error = exception_object(
        caller.call(
            &method,
            Value::Object(receiver),
            &[Value::Object(throwing_input)],
        ),
        "caller input TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "RegExp matchAll replaced a caller-realm user exception",
    );
}
