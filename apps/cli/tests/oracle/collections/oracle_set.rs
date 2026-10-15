use crate::object_graph_observation::{expect_object, global_callable};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{property_callable, take_exception_object};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Differential lock for the complete strong Set surface exposed by pinned
// QuickJS 2026-06-04. The vectors intentionally stay inside quickjs-oxide's
// implemented grammar and print only primitive ASCII observations.

#[test]
fn set_constructor_and_native_errors_use_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let defining_constructor = global_callable(&runtime, &mut defining, "Set");
    let defining_set_prototype =
        eval_object(&mut defining, "Set.prototype", "defining Set prototype");
    let caller_set_prototype = eval_object(&mut caller, "Set.prototype", "caller Set prototype");
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
    assert_ne!(defining_set_prototype, caller_set_prototype);
    assert_ne!(defining_type_error, caller_type_error);

    let foreign_set = expect_object(
        caller.construct(&defining_constructor, &[]).unwrap(),
        "foreign Set construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&foreign_set).unwrap(),
        Some(defining_set_prototype.try_clone().expect("duplicate root")),
        "Set construction did not use the foreign constructor prototype",
    );

    let caller_new_target = eval_callable(
        &runtime,
        &mut caller,
        "(function(){function F(){}F.prototype=17;return F})()",
        "caller primitive-prototype newTarget",
    );
    let fallback_set = expect_object(
        caller
            .construct_with_new_target(&defining_constructor, &caller_new_target, &[])
            .unwrap(),
        "cross-realm Set fallback construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&fallback_set).unwrap(),
        Some(caller_set_prototype),
        "primitive newTarget.prototype did not fall back to the newTarget realm",
    );

    let add = property_callable(&runtime, &mut defining, &defining_set_prototype, "add");
    let added = caller
        .call(&add, Value::Object(foreign_set), &[Value::Int(1)])
        .unwrap();
    assert!(matches!(added, Value::Object(_)));

    let ordinary = eval_object(&mut caller, "({})", "caller ordinary object");
    assert_eq!(
        caller.call(&add, Value::Object(ordinary), &[Value::Int(1)]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "foreign Set.add TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Set branded native error used the calling realm",
    );
}
