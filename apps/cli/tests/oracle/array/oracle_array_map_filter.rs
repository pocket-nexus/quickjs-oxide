use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// This target pins the allocating modes of QuickJS 2026-06-04's shared
// `js_array_every` kernel and its `JS_ArraySpeciesCreate` boundary.

#[test]
fn array_map_filter_species_boxing_results_and_errors_use_pinned_realms() {
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
    let caller_object_prototype = caller.object_prototype().unwrap();
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let map = property_callable(&runtime, &mut defining, &defining_array_prototype, "map");
    let filter = property_callable(&runtime, &mut defining, &defining_array_prototype, "filter");
    let identity = eval_callable(
        &runtime,
        &mut caller,
        "(function(value,index,receiver){globalThis.mapReceiver=receiver;return value})",
        "caller map identity callback",
    );

    let source = eval_object(&mut caller, "[1]", "caller Array");
    let Value::Object(mapped) = caller
        .call(
            &map,
            Value::Object(source.try_clone().expect("duplicate root")),
            &[Value::Object(
                identity.as_object().try_clone().expect("duplicate root"),
            )],
        )
        .expect("cross-realm default Array.map")
    else {
        panic!("cross-realm Array.map did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&mapped).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "cross-realm default Array constructor was not replaced by the method realm Array",
    );

    let always = eval_callable(
        &runtime,
        &mut caller,
        "(function(){return true})",
        "caller filter callback",
    );
    let Value::Object(filtered) = caller
        .call(
            &filter,
            Value::Object(source),
            &[Value::Object(
                always.as_object().try_clone().expect("duplicate root"),
            )],
        )
        .expect("cross-realm default Array.filter")
    else {
        panic!("cross-realm Array.filter did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&filtered).unwrap(),
        Some(defining_array_prototype),
        "cross-realm default Array.filter result used the caller realm",
    );

    drop(
        caller
            .call(
                &map,
                Value::String(JsString::try_from_utf8("a").unwrap()),
                &[Value::Object(
                    identity.as_object().try_clone().expect("duplicate root"),
                )],
            )
            .expect("cross-realm primitive Array.map"),
    );
    let boxed = eval_object(&mut caller, "mapReceiver", "captured map receiver");
    assert_eq!(
        runtime.get_prototype_of(&boxed).unwrap(),
        Some(defining_string_prototype),
        "Array.map boxed its primitive receiver in the callback realm",
    );

    drop(
        caller
            .eval("globalThis.callbackObject=Object()")
            .expect("install caller callback object"),
    );
    let object_callback = eval_callable(
        &runtime,
        &mut caller,
        "(function(){return callbackObject})",
        "caller object-producing callback",
    );
    let one = eval_object(&mut caller, "[1]", "caller callback source");
    let Value::Object(result) = caller
        .call(
            &map,
            Value::Object(one),
            &[Value::Object(
                object_callback
                    .as_object()
                    .try_clone()
                    .expect("duplicate root"),
            )],
        )
        .expect("cross-realm object-valued Array.map")
    else {
        panic!("object-valued Array.map did not return an object");
    };
    let zero = runtime.intern_property_key("0").unwrap();
    let Value::Object(callback_result) = caller.get_property(&result, &zero).unwrap() else {
        panic!("Array.map did not retain the callback object");
    };
    assert_eq!(
        runtime.get_prototype_of(&callback_result).unwrap(),
        Some(caller_object_prototype.try_clone().expect("duplicate root")),
        "Array.map moved a callback result into the method defining realm",
    );
    let custom_source = eval_object(
        &mut caller,
        "(function(){var source=[1],ctor=Object();ctor[Symbol.species]=function(length){globalThis.crossSpeciesLength=length;return Object()};source.constructor=ctor;return source})()",
        "caller Array with custom species",
    );
    let Value::Object(custom_result) = caller
        .call(
            &map,
            Value::Object(custom_source),
            &[Value::Object(
                identity.as_object().try_clone().expect("duplicate root"),
            )],
        )
        .expect("cross-realm custom species Array.map")
    else {
        panic!("custom species Array.map did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&custom_result).unwrap(),
        Some(caller_object_prototype),
        "Array.map moved a custom species result into the method realm",
    );
    assert_eq!(
        caller.eval("crossSpeciesLength").unwrap(),
        Value::Int(1),
        "Array.map passed the wrong length to cross-realm species",
    );

    let bad = eval_object(
        &mut caller,
        "(function(){var source=[1];source.constructor=1;return source})()",
        "caller Array with primitive constructor",
    );
    assert!(matches!(
        caller.call(
            &map,
            Value::Object(bad),
            &[Value::Object(
                identity.as_object().try_clone().expect("duplicate root")
            )],
        ),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.map species TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.map species TypeError did not use the method defining realm",
    );

    let throwing = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw new TypeError('caller callback')})",
        "caller throwing map callback",
    );
    let throwing_source = eval_object(&mut caller, "[1]", "caller throwing source");
    assert!(matches!(
        caller.call(
            &map,
            Value::Object(throwing_source),
            &[Value::Object(
                throwing.as_object().try_clone().expect("duplicate root")
            )],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.map callback TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.map replaced a callback throw with a defining-realm error",
    );
}
