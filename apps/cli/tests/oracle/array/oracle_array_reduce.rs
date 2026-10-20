use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::core::{JsString, Value};

// This target pins QuickJS 2026-06-04's shared `js_array_reduce` kernel:
// reduce and reduceRight, including their omitted-initial-value scan.

#[test]
fn array_reduce_boxing_accumulators_and_errors_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let defining_string_prototype = defining.string_prototype().unwrap();
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
    let caller_object_prototype = caller.object_prototype().unwrap();
    let reduce = property_callable(&runtime, &mut defining, &defining_array_prototype, "reduce");

    let capture = eval_callable(
        &runtime,
        &mut caller,
        "(function(acc,value,index,receiver){globalThis.reduceReceiver=receiver;return acc+value})",
        "caller receiver-capturing reduce callback",
    );
    assert_eq!(
        caller
            .call(
                &reduce,
                Value::String(JsString::try_from_utf8("a").unwrap()),
                &[
                    Value::Object(capture.as_object().try_clone().expect("duplicate root")),
                    Value::String(JsString::try_from_utf8("").unwrap()),
                ],
            )
            .expect("cross-realm primitive Array.reduce call"),
        Value::String(JsString::try_from_utf8("a").unwrap()),
    );
    let captured = eval_object(&mut caller, "reduceReceiver", "captured reduce receiver");
    assert_eq!(
        runtime.get_prototype_of(&captured).unwrap(),
        Some(defining_string_prototype),
        "Array.reduce boxed its callback receiver in the caller realm",
    );

    let empty = eval_object(&mut caller, "[]", "caller empty Array");
    let invalid_callback = exception_object(
        caller.call(
            &reduce,
            Value::Object(empty.try_clone().expect("duplicate root")),
            &[Value::Int(0)],
        ),
        "Array.reduce callback TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&invalid_callback).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "Array.reduce callback TypeError did not use the method defining realm",
    );

    let identity = eval_callable(
        &runtime,
        &mut caller,
        "(function(acc){return acc})",
        "caller identity reduce callback",
    );
    let empty_error = exception_object(
        caller.call(
            &reduce,
            Value::Object(empty.try_clone().expect("duplicate root")),
            &[Value::Object(
                identity.as_object().try_clone().expect("duplicate root"),
            )],
        ),
        "Array.reduce empty TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&empty_error).unwrap(),
        Some(defining_type_error),
        "Array.reduce empty TypeError did not use the method defining realm",
    );

    let marker = eval_object(&mut caller, "Object()", "caller initial accumulator");
    assert_eq!(
        caller
            .call(
                &reduce,
                Value::Object(empty),
                &[
                    Value::Object(identity.as_object().try_clone().expect("duplicate root")),
                    Value::Object(marker.try_clone().expect("duplicate root")),
                ],
            )
            .expect("empty reduce with explicit accumulator"),
        Value::Object(marker),
        "Array.reduce replaced an untouched caller-realm accumulator",
    );

    let throwing = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw new TypeError('caller callback')})",
        "caller throwing reduce callback",
    );
    let one = eval_object(&mut caller, "[1]", "caller one-element Array");
    let user_error = exception_object(
        caller.call(
            &reduce,
            Value::Object(one.try_clone().expect("duplicate root")),
            &[
                Value::Object(throwing.as_object().try_clone().expect("duplicate root")),
                Value::Int(0),
            ],
        ),
        "Array.reduce user callback error",
    );
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.reduce replaced a user callback throw with a defining-realm error",
    );

    let make_object = eval_callable(
        &runtime,
        &mut caller,
        "(function(){return Object()})",
        "caller object-producing reduce callback",
    );
    let Value::Object(produced) = caller
        .call(
            &reduce,
            Value::Object(one),
            &[
                Value::Object(make_object.as_object().try_clone().expect("duplicate root")),
                Value::Int(0),
            ],
        )
        .expect("cross-realm callback-produced accumulator")
    else {
        panic!("Array.reduce callback did not return its object accumulator");
    };
    assert_eq!(
        runtime.get_prototype_of(&produced).unwrap(),
        Some(caller_object_prototype),
        "Array.reduce moved a callback-produced accumulator into the defining realm",
    );
}
