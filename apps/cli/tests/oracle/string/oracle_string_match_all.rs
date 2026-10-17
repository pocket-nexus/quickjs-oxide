use crate::object_graph_observation::{
    callable_property, checked_property_value as object_property,
};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::string_value;
use crate::runtime_observation::take_exception_object;
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::core::{RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04 `js_string_match`'s
// Symbol.matchAll path and `check_regexp_g_flag` (`quickjs.c` 45583-45657).

#[test]
fn string_match_all_delegation_and_fallback_preserve_cross_realm_ownership() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let method = eval_callable(
        &runtime,
        &mut defining,
        "String.prototype.matchAll",
        "defining String matchAll",
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
    let defining_array_prototype =
        eval_object(&mut defining, "Array.prototype", "defining Array prototype");
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_iterator_prototype = eval_object(
        &mut caller,
        "Object.getPrototypeOf(/x/g[Symbol.matchAll](\"\"))",
        "caller RegExp String Iterator prototype",
    );
    let caller_object_prototype =
        eval_object(&mut caller, "Object.prototype", "caller Object prototype");
    let caller_array_prototype =
        eval_object(&mut caller, "Array.prototype", "caller Array prototype");
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );

    let Value::Object(fallback_iterator) = caller
        .call(&method, string_value("ba"), &[string_value("a")])
        .expect("cross-realm primitive String matchAll fallback")
    else {
        panic!("String matchAll primitive fallback did not return an iterator");
    };
    assert_eq!(
        runtime.get_prototype_of(&fallback_iterator).unwrap(),
        Some(defining_iterator_prototype),
        "String matchAll fallback did not use its defining RegExp intrinsic",
    );
    let fallback_next = callable_property(&runtime, &mut caller, &fallback_iterator, "next");
    let Value::Object(fallback_step) = caller
        .call(
            &fallback_next,
            Value::Object(fallback_iterator.try_clone().expect("duplicate root")),
            &[],
        )
        .expect("fallback iterator next")
    else {
        panic!("fallback iterator next did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&fallback_step).unwrap(),
        Some(defining_object_prototype),
        "fallback iterator result wrapper did not use the defining realm",
    );
    let Value::Object(fallback_match) =
        object_property(&runtime, &mut caller, &fallback_step, "value")
    else {
        panic!("fallback iterator did not yield a match object");
    };
    assert_eq!(
        runtime.get_prototype_of(&fallback_match).unwrap(),
        Some(defining_array_prototype),
        "fallback RegExp match result did not use the defining realm Array",
    );

    let foreign_regexp = eval_object(&mut caller, "/a/g", "caller global RegExp");
    let Value::Object(delegated_iterator) = caller
        .call(
            &method,
            string_value("ba"),
            &[Value::Object(foreign_regexp)],
        )
        .expect("cross-realm delegated String matchAll")
    else {
        panic!("delegated String matchAll did not return an iterator");
    };
    assert_eq!(
        runtime.get_prototype_of(&delegated_iterator).unwrap(),
        Some(caller_iterator_prototype),
        "String matchAll did not return the foreign RegExp hook result unchanged",
    );
    let delegated_next = callable_property(&runtime, &mut caller, &delegated_iterator, "next");
    let Value::Object(delegated_step) = caller
        .call(
            &delegated_next,
            Value::Object(delegated_iterator.try_clone().expect("duplicate root")),
            &[],
        )
        .expect("delegated iterator next")
    else {
        panic!("delegated iterator next did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&delegated_step).unwrap(),
        Some(caller_object_prototype),
        "delegated iterator result wrapper moved into the String method realm",
    );
    let Value::Object(delegated_match) =
        object_property(&runtime, &mut caller, &delegated_step, "value")
    else {
        panic!("delegated iterator did not yield a match object");
    };
    assert_eq!(
        runtime.get_prototype_of(&delegated_match).unwrap(),
        Some(caller_array_prototype),
        "delegated foreign RegExp match result moved into the String method realm",
    );

    let nonglobal = eval_object(&mut caller, "/a/", "caller non-global RegExp");
    assert_eq!(
        caller.call(&method, string_value("a"), &[Value::Object(nonglobal)],),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "String matchAll g-check TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "String matchAll g-check TypeError did not use the method realm",
    );

    let throwing_pattern = eval_object(
        &mut caller,
        r#"(function(){
            var pattern=Object();
            pattern[Symbol.matchAll]=function(){return null};
            pattern[Symbol.match]=true;
            Object.defineProperty(pattern,"flags",{get:function(){
                throw new TypeError("caller flags");
            }});
            return pattern;
        })()"#,
        "caller throwing matchAll pattern",
    );
    assert_eq!(
        caller.call(
            &method,
            string_value("a"),
            &[Value::Object(throwing_pattern)],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller flags TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "String matchAll replaced a caller-realm flags exception",
    );
}
