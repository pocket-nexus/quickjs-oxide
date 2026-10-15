use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::{RuntimeError, Value};

// This target pins QuickJS 2026-06-04's shared `js_array_find` kernel. Unlike
// indexOf, every index is read and passed to the predicate, including holes.

#[test]
fn array_find_native_errors_and_user_throws_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_object_prototype =
        eval_object(&mut caller, "Object.prototype", "caller Object prototype");
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let find = property_callable(&runtime, &mut defining, &defining_array_prototype, "find");
    let find_index = property_callable(
        &runtime,
        &mut defining,
        &defining_array_prototype,
        "findIndex",
    );

    let receiver = eval_object(&mut caller, "[Object()]", "caller object element Array");
    let always = eval_callable(
        &runtime,
        &mut caller,
        "(function(){return true})",
        "caller true predicate",
    );
    let Value::Object(found) = caller
        .call(
            &find,
            Value::Object(receiver),
            &[Value::Object(
                always.as_object().try_clone().expect("duplicate root"),
            )],
        )
        .expect("cross-realm Array.find call")
    else {
        panic!("cross-realm Array.find result was not the caller object element");
    };
    assert_eq!(
        runtime.get_prototype_of(&found).unwrap(),
        Some(caller_object_prototype),
        "Array.find replaced a caller-realm element",
    );

    let indexed_receiver = eval_object(&mut caller, "[10,20]", "caller indexed Array");
    let twenty = eval_callable(
        &runtime,
        &mut caller,
        "(function(value){return value===20})",
        "caller value predicate",
    );
    assert_eq!(
        caller
            .call(
                &find_index,
                Value::Object(indexed_receiver),
                &[Value::Object(
                    twenty.as_object().try_clone().expect("duplicate root")
                )],
            )
            .expect("cross-realm Array.findIndex call"),
        Value::Int(1),
    );

    let empty = eval_object(&mut caller, "[]", "caller empty Array");
    assert!(matches!(
        caller.call(&find, Value::Object(empty), &[Value::Int(0)]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.find predicate TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.find native TypeError did not use the method defining realm",
    );

    let throwing = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw new TypeError('caller predicate')})",
        "caller throwing predicate",
    );
    let one = eval_object(&mut caller, "[1]", "caller one-element Array");
    assert!(matches!(
        caller.call(
            &find,
            Value::Object(one),
            &[Value::Object(
                throwing.as_object().try_clone().expect("duplicate root")
            )],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.find user predicate error");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.find replaced a user predicate throw with a defining-realm error",
    );
}
