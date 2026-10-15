use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::{
    checked_property_value as value_property, eval_described_object as eval_object,
    eval_named_callable as eval_callable, require_object as expect_object,
};
use quickjs_oxide::engine::api::{Context, JsString, ObjectRef, Runtime, Value};

// Differential lock for pinned QuickJS 2026-06-04 RegExp match indices.
//
// These vectors keep `d` observable all the way through the consumers that
// either preserve the `indices` graph (`exec`, non-global match, and matchAll)
// or intentionally discard it (global match, split, and replacement).  They
// also pin QuickJS's UTF-16 `lastIndex` behavior when execution starts on the
// low surrogate of a pair.

#[test]
fn regexp_match_indices_nested_arrays_use_the_exec_defining_realm() {
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
        "new RegExp('(?<x>a)(b)?','d')",
        "caller match-indices RegExp",
    );
    let result = expect_object(
        caller
            .call(
                &exec,
                Value::Object(regexp),
                &[Value::String(JsString::try_from_utf8("za").unwrap())],
            )
            .expect("cross-realm match-indices exec"),
        "cross-realm match-indices result",
    );
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype.try_clone().expect("duplicate root")),
    );

    let indices = object_property(&runtime, &mut caller, &result, "indices");
    assert_eq!(
        runtime.get_prototype_of(&indices).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
    );
    let whole_pair = object_property(&runtime, &mut caller, &indices, "0");
    let named_pair = object_property(&runtime, &mut caller, &indices, "1");
    for pair in [&whole_pair, &named_pair] {
        assert_eq!(
            runtime.get_prototype_of(pair).unwrap(),
            Some(
                defining_array_prototype
                    .try_clone()
                    .expect("duplicate root")
            ),
        );
        assert_ne!(
            runtime.get_prototype_of(pair).unwrap(),
            Some(caller_array_prototype.try_clone().expect("duplicate root")),
        );
    }
    assert_eq!(
        value_property(&runtime, &mut caller, &indices, "2"),
        Value::Undefined,
    );

    let groups = object_property(&runtime, &mut caller, &indices, "groups");
    assert_eq!(runtime.get_prototype_of(&groups).unwrap(), None);
    assert_eq!(
        value_property(&runtime, &mut caller, &groups, "x"),
        Value::Object(named_pair),
        "indices.groups did not alias the participating pair",
    );
}

fn object_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> ObjectRef {
    expect_object(value_property(runtime, context, object, name), name)
}
