use crate::object_graph_observation::{expect_object, global_callable};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{property_callable, take_exception_object};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Differential lock for the complete strong Map surface exposed by pinned
// QuickJS 2026-06-04. The vectors intentionally stay inside quickjs-oxide's
// implemented grammar and print only primitive ASCII observations.

#[test]
fn map_constructor_and_native_errors_use_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let defining_constructor = global_callable(&runtime, &mut defining, "Map");
    let defining_map_prototype =
        eval_object(&mut defining, "Map.prototype", "defining Map prototype");
    let caller_map_prototype = eval_object(&mut caller, "Map.prototype", "caller Map prototype");
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
    assert_ne!(defining_map_prototype, caller_map_prototype);
    assert_ne!(defining_type_error, caller_type_error);

    let foreign_map = expect_object(
        caller.construct(&defining_constructor, &[]).unwrap(),
        "foreign Map construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&foreign_map).unwrap(),
        Some(defining_map_prototype.try_clone().expect("duplicate root")),
        "Map construction did not use the foreign constructor prototype",
    );

    let caller_new_target = eval_callable(
        &runtime,
        &mut caller,
        "(function(){function F(){}F.prototype=17;return F})()",
        "caller primitive-prototype newTarget",
    );
    let fallback_map = expect_object(
        caller
            .construct_with_new_target(&defining_constructor, &caller_new_target, &[])
            .unwrap(),
        "cross-realm Map fallback construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&fallback_map).unwrap(),
        Some(caller_map_prototype),
        "primitive newTarget.prototype did not fall back to the newTarget realm",
    );

    let get = property_callable(&runtime, &mut defining, &defining_map_prototype, "get");
    assert!(matches!(
        caller.call(&get, Value::Object(foreign_map), &[Value::Int(1)]),
        Ok(Value::Undefined),
    ));

    let ordinary = eval_object(&mut caller, "({})", "caller ordinary object");
    assert_eq!(
        caller.call(&get, Value::Object(ordinary), &[Value::Int(1)]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "foreign Map.get TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Map branded native error used the calling realm",
    );
}
