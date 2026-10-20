use crate::object_graph_observation::{eval_object, global_callable, intrinsic_prototype};
use crate::runtime_observation::{host_runtime, host_runtime_context};

use crate::runtime_observation::{pending_exception_object as exception_object, property_callable};
use quickjs_oxide::engine::api::core::{JsString, Value};

// Pins QuickJS 2026-06-04 `js_object_assign` and `JS_CopyDataProperties`.
// Ordinary sources use QuickJS's enumerable-at-snapshot optimization; Proxy
// sources instead recheck each descriptor. Proxy and TypedArray exotic sources
// are locked below; module-namespace objects remain a separate frontier.
// Arguments is locked by its dedicated differential.

#[test]
fn object_assign_cross_realm_targets_and_error_realms_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let assign = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "assign",
    );

    let target = caller.new_object().unwrap();
    let source = eval_object(
        &mut caller,
        "(function(){var value=Object();value.x=1;return value})()",
    );
    let returned = caller
        .call(
            &assign,
            Value::Undefined,
            &[
                Value::Object(target.try_clone().expect("duplicate root")),
                Value::Object(source),
            ],
        )
        .unwrap();
    assert_eq!(
        returned,
        Value::Object(target.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        caller
            .get_property(&target, &runtime.intern_property_key("x").unwrap())
            .unwrap(),
        Value::Int(1),
    );

    let defining_number_prototype = intrinsic_prototype(&runtime, &mut defining, "Number");
    let source = eval_object(
        &mut caller,
        "(function(){var value=Object();value.x=2;return value})()",
    );
    let Value::Object(boxed) = caller
        .call(
            &assign,
            Value::Undefined,
            &[Value::Int(17), Value::Object(source)],
        )
        .unwrap()
    else {
        panic!("cross-realm primitive Object.assign target was not boxed");
    };
    assert_eq!(
        runtime.get_prototype_of(&boxed).unwrap(),
        Some(defining_number_prototype),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let framework_error = exception_object(caller.call(&assign, Value::Undefined, &[Value::Null]));
    assert_eq!(
        runtime.get_prototype_of(&framework_error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
    );

    let readonly = eval_object(
        &mut caller,
        r#"(function(){var target=Object(),descriptor=Object();descriptor.value=1;descriptor.writable=false;descriptor.enumerable=true;descriptor.configurable=true;Object.defineProperty(target,"x",descriptor);return target})()"#,
    );
    let readonly_source = eval_object(
        &mut caller,
        "(function(){var value=Object();value.x=2;return value})()",
    );
    let set_error = exception_object(caller.call(
        &assign,
        Value::Undefined,
        &[Value::Object(readonly), Value::Object(readonly_source)],
    ));
    assert_eq!(
        runtime.get_prototype_of(&set_error).unwrap(),
        Some(defining_type_error),
    );

    let caller_range_error = intrinsic_prototype(&runtime, &mut caller, "RangeError");
    let throwing = eval_object(
        &mut caller,
        r#"(function(){var value=Object();value.__defineGetter__("x",function(){throw new RangeError("source")});return value})()"#,
    );
    let user_error = exception_object(caller.call(
        &assign,
        Value::Undefined,
        &[Value::Object(target), Value::Object(throwing)],
    ));
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_range_error.try_clone().expect("duplicate root")),
    );

    let setter_target = eval_object(
        &mut caller,
        r#"(function(){var target=Object(),descriptor=Object();descriptor.enumerable=true;descriptor.configurable=true;descriptor.set=function(){throw new RangeError("target")};Object.defineProperty(target,"x",descriptor);return target})()"#,
    );
    let setter_source = eval_object(
        &mut caller,
        "(function(){var value=Object();value.x=3;return value})()",
    );
    let setter_error = exception_object(caller.call(
        &assign,
        Value::Undefined,
        &[Value::Object(setter_target), Value::Object(setter_source)],
    ));
    assert_eq!(
        runtime.get_prototype_of(&setter_error).unwrap(),
        Some(caller_range_error),
    );
}

#[test]
fn object_assign_exotic_source_families_are_published() {
    let (_runtime, context) = host_runtime_context();
    assert_eq!(
        context
            .eval("typeof Proxy+'|'+typeof ArrayBuffer+'|'+typeof Uint8Array")
            .unwrap(),
        Value::String(JsString::try_from_utf8("function|function|function").unwrap()),
        "the active exotic Object.assign vectors require these source families",
    );
}
