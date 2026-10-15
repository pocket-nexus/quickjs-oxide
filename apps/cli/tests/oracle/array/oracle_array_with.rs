use super::support::*;
use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::int_property;
use quickjs_oxide::engine::api::{RuntimeError, Value};

// This target pins QuickJS 2026-06-04 `Array.prototype.with` as one complete
// change-by-copy slice. Source probes deliberately avoid later Array methods
// and later Object reflection APIs on the Rust side. The graph probe uses the
// host API so lazy native descriptors remain observable without expanding the
// source-level Object surface.

#[test]
fn array_with_result_and_native_errors_use_the_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let caller_array_prototype = caller.array_prototype().unwrap();
    let defining_range_error = eval_object(
        &mut defining,
        "RangeError.prototype",
        "RangeError prototype",
    );
    let caller_type_error = eval_object(&mut caller, "TypeError.prototype", "TypeError prototype");
    let method = property_callable(&runtime, &mut defining, &defining_array_prototype, "with");

    let receiver = eval_object(&mut caller, "[10,20]", "caller Array receiver");
    let Value::Object(result) = caller
        .call(
            &method,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[Value::Int(1), Value::Int(99)],
        )
        .expect("cross-realm Array.with call")
    else {
        panic!("cross-realm Array.with result was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "Array.with result did not use the native defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype),
    );
    assert_eq!(int_property(&runtime, &mut caller, &result, "0"), 10);
    assert_eq!(int_property(&runtime, &mut caller, &result, "1"), 99);

    assert!(matches!(
        caller.call(
            &method,
            Value::Object(receiver),
            &[Value::Int(2), Value::Int(0)],
        ),
        Err(RuntimeError::Exception),
    ));
    let range_error = take_exception_object(&mut caller, "cross-realm Array.with RangeError");
    assert_eq!(
        runtime.get_prototype_of(&range_error).unwrap(),
        Some(defining_range_error),
        "Array.with RangeError did not use the native defining realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var source=Object();source.length=2;
            source.__defineGetter__("0",function(){throw new TypeError("caller getter")});
            return source;
        })()"#,
        "caller throwing receiver",
    );
    assert!(matches!(
        caller.call(
            &method,
            Value::Object(throwing_receiver),
            &[Value::Int(1), Value::Int(0)],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.with user getter error");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.with replaced a user getter throw with a defining-realm error",
    );
}
