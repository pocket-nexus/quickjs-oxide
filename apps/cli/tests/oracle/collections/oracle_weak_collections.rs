use crate::object_graph_observation::{expect_object, global_callable};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{property_callable, take_exception_object};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{Context, Runtime, RuntimeError, Value};

// Differential lock for the WeakMap and WeakSet surface exposed by pinned
// QuickJS 2026-06-04. The vectors use only primitive ASCII observations so
// they can be compared without sharing object identities across engines.

#[test]
fn weak_collection_new_target_fallback_and_native_errors_use_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let map_key = Value::Object(eval_object(&mut caller, "({})", "caller WeakMap key"));
    assert_cross_realm_collection(
        &runtime,
        &mut defining,
        &mut caller,
        "WeakMap",
        "get",
        &[map_key],
    );
    let set_key = Value::Object(eval_object(&mut caller, "({})", "caller WeakSet key"));
    assert_cross_realm_collection(
        &runtime,
        &mut defining,
        &mut caller,
        "WeakSet",
        "has",
        &[set_key],
    );
}

fn assert_cross_realm_collection(
    runtime: &Runtime,
    defining: &mut Context,
    caller: &mut Context,
    constructor_name: &str,
    method_name: &str,
    arguments: &[Value],
) {
    let defining_constructor = global_callable(runtime, defining, constructor_name);
    let defining_prototype = eval_object(
        defining,
        &format!("{constructor_name}.prototype"),
        "defining weak collection prototype",
    );
    let caller_prototype = eval_object(
        caller,
        &format!("{constructor_name}.prototype"),
        "caller weak collection prototype",
    );
    let defining_type_error = eval_object(
        defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error =
        eval_object(caller, "TypeError.prototype", "caller TypeError prototype");
    assert_ne!(defining_prototype, caller_prototype);
    assert_ne!(defining_type_error, caller_type_error);

    let foreign_collection = expect_object(
        caller.construct(&defining_constructor, &[]).unwrap(),
        "foreign weak collection construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&foreign_collection).unwrap(),
        Some(defining_prototype.try_clone().expect("duplicate root")),
        "{constructor_name} construction did not use the foreign constructor prototype",
    );

    let caller_new_target = eval_callable(
        runtime,
        caller,
        "(function(){function F(){}F.prototype=17;return F})()",
        "caller primitive-prototype newTarget",
    );
    let fallback_collection = expect_object(
        caller
            .construct_with_new_target(&defining_constructor, &caller_new_target, &[])
            .unwrap(),
        "cross-realm weak collection fallback construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&fallback_collection).unwrap(),
        Some(caller_prototype),
        "primitive newTarget.prototype did not fall back to the newTarget realm {constructor_name}",
    );

    let method = property_callable(runtime, defining, &defining_prototype, method_name);
    let ordinary = eval_object(caller, "({})", "caller ordinary object");
    assert_eq!(
        caller.call(&method, Value::Object(ordinary), arguments),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(caller, "foreign weak collection brand TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "{constructor_name} branded native error used the calling realm",
    );
}
