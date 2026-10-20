use super::support::*;
use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::int_property;
use quickjs_oxide::engine::api::core::Value;

// This target pins QuickJS 2026-06-04 `Array.prototype.copyWithin`, including
// overlap direction and the hole-to-throwing-Delete path in `JS_CopySubArray`.

#[test]
fn array_copy_within_boxing_native_errors_and_user_throws_use_pinned_realms() {
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
    let method = property_callable(
        &runtime,
        &mut defining,
        &defining_array_prototype,
        "copyWithin",
    );

    let receiver = eval_object(&mut caller, "[10,20,30]", "caller Array receiver");
    let Value::Object(result) = caller
        .call(
            &method,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[Value::Int(0), Value::Int(1)],
        )
        .expect("cross-realm Array.copyWithin call")
    else {
        panic!("cross-realm Array.copyWithin result was not an object");
    };
    assert_eq!(
        result, receiver,
        "Array.copyWithin did not return its receiver"
    );
    assert_eq!(int_property(&runtime, &mut caller, &result, "0"), 20);
    assert_eq!(int_property(&runtime, &mut caller, &result, "1"), 30);

    let Value::Object(wrapper) = caller
        .call(&method, Value::Bool(true), &[Value::Int(0), Value::Int(0)])
        .expect("cross-realm primitive Array.copyWithin call")
    else {
        panic!("Array.copyWithin primitive result was not boxed");
    };
    assert_eq!(
        runtime.get_prototype_of(&wrapper).unwrap(),
        Some(defining_boolean_prototype),
        "Array.copyWithin boxed a primitive in the caller realm",
    );

    let readonly = eval_object(
        &mut caller,
        r#"(function(){
            var source=[0,1],descriptor=Object();
            descriptor.value=0;descriptor.writable=false;
            Object.defineProperty(source,"0",descriptor);return source;
        })()"#,
        "caller read-only copyWithin receiver",
    );
    let set_error = exception_object(
        caller.call(
            &method,
            Value::Object(readonly),
            &[Value::Int(0), Value::Int(1), Value::Int(2)],
        ),
        "Array.copyWithin Set TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&set_error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "Array.copyWithin Set TypeError did not use the method defining realm",
    );

    let undeletable = eval_object(
        &mut caller,
        r#"(function(){
            var source=[0,1],descriptor=Object();delete source[1];
            descriptor.value=0;descriptor.configurable=false;
            Object.defineProperty(source,"0",descriptor);return source;
        })()"#,
        "caller undeletable copyWithin receiver",
    );
    let delete_error = exception_object(
        caller.call(
            &method,
            Value::Object(undeletable),
            &[Value::Int(0), Value::Int(1), Value::Int(2)],
        ),
        "Array.copyWithin Delete TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&delete_error).unwrap(),
        Some(defining_type_error),
        "Array.copyWithin Delete TypeError did not use the method defining realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var source=Object();source.length=1;
            source.__defineGetter__("0",function(){throw new TypeError("caller getter")});
            return source;
        })()"#,
        "caller throwing copyWithin receiver",
    );
    let user_error = exception_object(
        caller.call(
            &method,
            Value::Object(throwing_receiver),
            &[Value::Int(0), Value::Int(0), Value::Int(1)],
        ),
        "Array.copyWithin user getter error",
    );
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.copyWithin replaced a user getter throw with a defining-realm error",
    );
}
