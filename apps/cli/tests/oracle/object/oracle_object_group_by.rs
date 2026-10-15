use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::{
    eval_object, eval_source_callable as eval_callable, global_callable, int_property,
    object_property,
};
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Pins the QuickJS 2026-06-04 `Object.groupBy` table entry and the shared
// `js_object_groupBy(..., is_map = 0)` iterator kernel.

#[test]
fn object_group_by_uses_its_defining_realm_and_preserves_user_throws() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let object = global_callable(&runtime, &mut defining, "Object");
    let group_by = property_callable(&runtime, &mut defining, object.as_object(), "groupBy");
    let source = eval_object(&mut caller, "[1,2]");
    let callback = eval_callable(
        &runtime,
        &mut caller,
        r#"(function(value){
            "use strict";
            this.groupByMarker=(this.groupByMarker||0)+value;
            return "x";
        })"#,
    );

    let Value::Object(groups) = caller
        .call(
            &group_by,
            Value::Undefined,
            &[
                Value::Object(source),
                Value::Object(callback.as_object().try_clone().expect("duplicate root")),
            ],
        )
        .expect("cross-realm Object.groupBy")
    else {
        panic!("Object.groupBy did not return an object");
    };
    assert_eq!(runtime.get_prototype_of(&groups).unwrap(), None);
    let group = object_property(&runtime, &mut caller, &groups, "x");
    assert_eq!(
        runtime.get_prototype_of(&group).unwrap().as_ref(),
        Some(&defining.array_prototype().unwrap()),
        "group Array did not use Object.groupBy's defining realm",
    );
    let defining_global = defining.global_object().unwrap();
    assert_eq!(
        int_property(&runtime, &mut defining, &defining_global, "groupByMarker",),
        3,
    );
    let caller_global = caller.global_object().unwrap();
    assert!(matches!(
        caller
            .get_property(
                &caller_global,
                &runtime.intern_property_key("groupByMarker").unwrap(),
            )
            .unwrap(),
        Value::Undefined
    ));

    let defining_type_error = eval_object(&mut defining, "TypeError.prototype");
    let empty = caller.new_array().unwrap();
    assert!(matches!(
        caller.call(
            &group_by,
            Value::Undefined,
            &[Value::Object(empty), Value::Int(0)],
        ),
        Err(RuntimeError::Exception)
    ));
    let Value::Object(error) = caller
        .take_exception()
        .unwrap()
        .expect("missing Object.groupBy native error")
    else {
        panic!("Object.groupBy native error was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap().as_ref(),
        Some(&defining_type_error),
        "native callback validation error used the caller realm",
    );

    let sentinel = caller.new_object().unwrap();
    let sentinel_key = runtime.intern_property_key("groupBySentinel").unwrap();
    assert!(
        caller
            .set_property(
                &caller.global_object().unwrap(),
                &sentinel_key,
                Value::Object(sentinel.try_clone().expect("duplicate root")),
            )
            .unwrap()
    );
    let throwing = eval_callable(&runtime, &mut caller, "(function(){throw groupBySentinel})");
    let singleton = caller.new_array_from_values(vec![Value::Int(1)]).unwrap();
    assert!(matches!(
        caller.call(
            &group_by,
            Value::Undefined,
            &[
                Value::Object(singleton),
                Value::Object(throwing.as_object().try_clone().expect("duplicate root")),
            ],
        ),
        Err(RuntimeError::Exception)
    ));
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "callback throw identity was not preserved",
    );
}
