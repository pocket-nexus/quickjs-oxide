use crate::object_graph_observation::object_property;
use crate::object_graph_observation::{
    eval_described_object as eval_object, eval_named_callable as eval_callable,
    require_object as expect_object,
};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::string_property_with_read_context as string_property;
use quickjs_oxide::engine::api::Value;

// Differential lock for pinned QuickJS 2026-06-04 named RegExp captures.
//
// The vectors deliberately cover the QuickJS implementation, including its
// observable parser quirks: named-capture scopes are a wrapping global u8
// advanced by every alternative, and the lexical forward-reference scan skips
// the first byte in a named capture body.  Runtime cases pin the capture-name
// trailer consumers (`groups`, `indices.groups`, and replacement) rather than
// testing only the matcher in isolation.

#[test]
fn regexp_named_group_results_use_the_exec_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype =
        eval_object(&mut defining, "Array.prototype", "defining Array prototype");
    let caller_array_prototype =
        eval_object(&mut caller, "Array.prototype", "caller Array prototype");
    let exec = eval_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype.exec",
        "defining RegExp exec",
    );
    let regexp = eval_object(
        &mut caller,
        "new RegExp('(?<x>a)','d')",
        "caller named RegExp",
    );
    let result = expect_object(
        caller
            .call(
                &exec,
                Value::Object(regexp),
                &[Value::String(
                    quickjs_oxide::engine::api::JsString::try_from_utf8("za").unwrap(),
                )],
            )
            .expect("cross-realm named RegExp exec"),
        "cross-realm named RegExp result",
    );
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(defining_array_prototype),
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype),
    );

    let groups = object_property(&runtime, &mut caller, &result, "groups");
    assert_eq!(runtime.get_prototype_of(&groups).unwrap(), None);
    assert_eq!(string_property(&runtime, &mut caller, &groups, "x"), "a",);
    let indices = object_property(&runtime, &mut caller, &result, "indices");
    assert_eq!(
        runtime.get_prototype_of(&indices).unwrap(),
        Some(eval_object(
            &mut defining,
            "Array.prototype",
            "defining Array prototype after exec",
        )),
    );
    let indices_groups = object_property(&runtime, &mut caller, &indices, "groups");
    assert_eq!(runtime.get_prototype_of(&indices_groups).unwrap(), None);
}
