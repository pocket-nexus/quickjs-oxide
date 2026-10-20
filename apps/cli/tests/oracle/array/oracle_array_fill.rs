use super::support::*;
use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::int_property;
use quickjs_oxide::engine::api::core::Value;

// This target pins QuickJS 2026-06-04 `Array.prototype.fill` as one complete
// generic mutation slice. The probes emphasize QuickJS's exact conversion and
// throwing-Set order, including partial mutation after an indexed write fails.

#[test]
fn array_fill_boxing_native_errors_and_user_throws_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let defining_boolean_prototype = eval_object(
        &mut defining,
        "Boolean.prototype",
        "defining Boolean prototype",
    );
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
    let method = property_callable(&runtime, &mut defining, &defining_array_prototype, "fill");

    let receiver = eval_object(&mut caller, "[10,20]", "caller Array receiver");
    let Value::Object(result) = caller
        .call(
            &method,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[Value::Int(99), Value::Int(1)],
        )
        .expect("cross-realm Array.fill call")
    else {
        panic!("cross-realm Array.fill result was not an object");
    };
    assert_eq!(result, receiver, "Array.fill did not return its receiver");
    assert_eq!(int_property(&runtime, &mut caller, &result, "0"), 10);
    assert_eq!(int_property(&runtime, &mut caller, &result, "1"), 99);

    let Value::Object(wrapper) = caller
        .call(&method, Value::Bool(true), &[])
        .expect("cross-realm primitive Array.fill call")
    else {
        panic!("Array.fill primitive result was not boxed");
    };
    assert_eq!(
        runtime.get_prototype_of(&wrapper).unwrap(),
        Some(defining_boolean_prototype),
        "Array.fill boxed a primitive in the caller realm",
    );

    let readonly = eval_object(
        &mut caller,
        r#"(function(){
            var source=[1],descriptor=Object();
            descriptor.value=1;descriptor.writable=false;
            Object.defineProperty(source,"0",descriptor);return source;
        })()"#,
        "caller read-only receiver",
    );
    let native_error = exception_object(
        caller.call(&method, Value::Object(readonly), &[Value::Int(9)]),
        "Array.fill native TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.fill native TypeError did not use the method defining realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var source=Object();source.length=1;
            source.__defineSetter__("0",function(){throw new TypeError("caller setter")});
            return source;
        })()"#,
        "caller throwing receiver",
    );
    let user_error = exception_object(
        caller.call(&method, Value::Object(throwing_receiver), &[Value::Int(9)]),
        "Array.fill user setter error",
    );
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.fill replaced a user setter throw with a defining-realm error",
    );
}
