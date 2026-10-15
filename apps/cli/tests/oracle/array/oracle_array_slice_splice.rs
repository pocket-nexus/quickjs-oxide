use super::support::*;
use crate::runtime_observation::{host_runtime, host_runtime_context};

use crate::object_graph_observation::int_property;
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// This target pins QuickJS 2026-06-04 `js_array_slice`, its magic-selected
// mutating `splice` branch, and the adjacent dense `js_array_toSpliced` path.

#[test]
fn array_slice_splice_basic_rust_smoke() {
    let (_runtime, mut context) = host_runtime_context();
    assert_eq!(
        context
            .eval(
                r#"(function(){
                    var source=[1,2,3,4],slice=source.slice(1,3);
                    var removed=source.splice(1,2,8,9,10),copy=source.toSpliced(1,1,7);
                    return slice.join(",")+"|"+removed.join(",")+"|"+
                        source.join(",")+"|"+copy.join(",");
                })()"#,
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("2,3|2,3|1,8,9,10,4|1,7,9,10,4").unwrap()),
    );
}

#[test]
fn array_slice_recursive_getter_stack_overflow_is_catchable_without_oracle() {
    let (_runtime, mut context) = host_runtime_context();
    assert_eq!(
        context
            .eval(
                r#"(function(){
                    var source=Object();source.length=1;
                    source.__defineGetter__("0",function(){
                        return Array.prototype.slice.call(source,0,1)[0];
                    });
                    try{Array.prototype.slice.call(source,0,1);return "missing"}
                    catch(error){return error.name+"|"+error.message}
                })()"#,
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("InternalError|stack overflow").unwrap()),
    );
}

#[test]
fn array_slice_splice_results_native_errors_and_user_throws_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let caller_array_prototype = caller.array_prototype().unwrap();
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let defining_range_error = eval_object(
        &mut defining,
        "RangeError.prototype",
        "defining RangeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let slice = property_callable(&runtime, &mut defining, &defining_array_prototype, "slice");
    let splice = property_callable(&runtime, &mut defining, &defining_array_prototype, "splice");
    let to_spliced = property_callable(
        &runtime,
        &mut defining,
        &defining_array_prototype,
        "toSpliced",
    );

    let receiver = eval_object(&mut caller, "[1,2,3]", "caller slice receiver");
    let Value::Object(result) = caller
        .call(
            &slice,
            Value::Object(receiver),
            &[Value::Int(1), Value::Int(3)],
        )
        .expect("cross-realm Array.slice")
    else {
        panic!("cross-realm Array.slice did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "foreign intrinsic Array constructor was not replaced by the method realm",
    );
    assert_eq!(int_property(&runtime, &mut caller, &result, "0"), 2);

    let receiver = eval_object(&mut caller, "[1,2,3]", "caller splice receiver");
    let Value::Object(removed) = caller
        .call(
            &splice,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[Value::Int(1), Value::Int(1), Value::Int(9)],
        )
        .expect("cross-realm Array.splice")
    else {
        panic!("cross-realm Array.splice did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&removed).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "Array.splice removed result did not use the method defining realm",
    );
    assert_eq!(int_property(&runtime, &mut caller, &removed, "0"), 2);
    assert_eq!(int_property(&runtime, &mut caller, &receiver, "1"), 9);

    let receiver = eval_object(&mut caller, "[1,2,3]", "caller toSpliced receiver");
    let Value::Object(result) = caller
        .call(
            &to_spliced,
            Value::Object(receiver),
            &[Value::Int(1), Value::Int(1), Value::Int(8)],
        )
        .expect("cross-realm Array.toSpliced")
    else {
        panic!("cross-realm Array.toSpliced did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "Array.toSpliced result did not use the method defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype.try_clone().expect("duplicate root")),
    );

    let custom_species = eval_object(
        &mut caller,
        "(function(){var source=[1,2],ctor=Object();function Species(length){return [length]}ctor[Symbol.species]=Species;source.constructor=ctor;return source})()",
        "caller custom-species source",
    );
    let Value::Object(custom_result) = caller
        .call(
            &slice,
            Value::Object(custom_species),
            &[Value::Int(0), Value::Int(1)],
        )
        .expect("cross-realm custom Array species")
    else {
        panic!("custom Array species did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&custom_result).unwrap(),
        Some(caller_array_prototype),
        "custom foreign species result lost its constructor realm",
    );

    let too_long = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=2147483648;return source})()",
        "caller oversized toSpliced receiver",
    );
    assert!(matches!(
        caller.call(&to_spliced, Value::Object(too_long), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.toSpliced RangeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_range_error),
        "Array.toSpliced RangeError did not use the method defining realm",
    );

    let overflow = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=9007199254740991;return source})()",
        "caller splice overflow receiver",
    );
    assert!(matches!(
        caller.call(
            &splice,
            Value::Object(overflow),
            &[Value::Int(0), Value::Int(0), Value::Int(1)],
        ),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.splice TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.splice TypeError did not use the method defining realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=1;source.__defineGetter__('0',function(){throw new TypeError('caller getter')});return source})()",
        "caller throwing slice receiver",
    );
    assert!(matches!(
        caller.call(
            &slice,
            Value::Object(throwing_receiver),
            &[Value::Int(0), Value::Int(1)],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.slice user getter TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.slice replaced a user getter throw with a defining-realm error",
    );
}
