use crate::object_graph_observation::object_property;
use crate::object_graph_observation::{eval_object, global_callable, intrinsic_prototype};
use crate::runtime_completion_oracle::observe_eval_completion as observe_rust_eval;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_object_getOwnPropertyDescriptor` and
// `js_object_getOwnPropertyDescriptors`. Proxy descriptor traps and TypedArray
// integer-indexed exotic descriptors participate in the same differential.
// Module namespace descriptors remain a separate module/object-model boundary.

const PROXY_GOPD_SAME_VALUE_OMISSION: &str = r#"(function(){
    var target=Object(),fixed=Object();
    fixed.value=2;fixed.writable=false;fixed.enumerable=true;fixed.configurable=false;
    Object.defineProperty(target,"x",fixed);
    var proxy=new Proxy(target,{getOwnPropertyDescriptor:function(){
        return {value:1,writable:false,enumerable:true,configurable:false};
    }});
    var result=Object.getOwnPropertyDescriptor(proxy,"x");
    return result.value+":"+result.writable+":"+result.enumerable+":"+result.configurable+"|"+target.x;
})()"#;

#[test]
fn object_descriptor_pins_quickjs_proxy_same_value_omission_without_an_oracle() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        observe_rust_eval(
            &runtime,
            &mut context,
            PROXY_GOPD_SAME_VALUE_OMISSION,
            "Proxy gOPD same-value omission",
        ),
        "return|string|1:false:true:false|2",
    );
}

#[test]
fn object_descriptor_cross_realm_results_nested_objects_and_errors_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object = global_callable(&runtime, &mut defining, "Object");
    let singular = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "getOwnPropertyDescriptor",
    );
    let plural = property_callable(
        &runtime,
        &mut defining,
        defining_object.as_object(),
        "getOwnPropertyDescriptors",
    );
    let defining_object_prototype = defining.object_prototype().unwrap();

    let source = eval_object(
        &mut caller,
        r#"(function(){var value=Object();value.x=1;return value})()"#,
    );
    let Value::Object(descriptor) = caller
        .call(
            &singular,
            Value::Undefined,
            &[
                Value::Object(source.try_clone().expect("duplicate root")),
                Value::String(JsString::try_from_utf8("x").unwrap()),
            ],
        )
        .unwrap()
    else {
        panic!("cross-realm Object.getOwnPropertyDescriptor did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&descriptor).unwrap(),
        Some(
            defining_object_prototype
                .try_clone()
                .expect("duplicate root")
        ),
    );

    let Value::Object(descriptors) = caller
        .call(&plural, Value::Undefined, &[Value::Object(source)])
        .unwrap()
    else {
        panic!("cross-realm Object.getOwnPropertyDescriptors did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&descriptors).unwrap(),
        Some(
            defining_object_prototype
                .try_clone()
                .expect("duplicate root")
        ),
    );
    let nested = object_property(&runtime, &mut caller, &descriptors, "x");
    assert_eq!(
        runtime.get_prototype_of(&nested).unwrap(),
        Some(defining_object_prototype),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    assert_eq!(
        caller.call(
            &singular,
            Value::Undefined,
            &[
                Value::Null,
                Value::String(JsString::try_from_utf8("x").unwrap()),
            ],
        ),
        Err(RuntimeError::Exception),
    );
    let framework_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&framework_error).unwrap(),
        Some(defining_type_error),
    );

    let caller_range_error = intrinsic_prototype(&runtime, &mut caller, "RangeError");
    let target = eval_object(&mut caller, "Object()");
    let throwing_key = eval_object(
        &mut caller,
        r#"(function(){var key=Object();key[Symbol.toPrimitive]=function(){throw new RangeError("key")};return key})()"#,
    );
    assert_eq!(
        caller.call(
            &singular,
            Value::Undefined,
            &[Value::Object(target), Value::Object(throwing_key)],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_range_error),
    );
}

#[test]
fn object_descriptor_methods_and_results_retain_then_release_their_realm() {
    let runtime = host_runtime();
    let (singular, plural, descriptors, nested) = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_object = global_callable(&runtime, &mut first, "Object");
        let second_object = global_callable(&runtime, &mut second, "Object");
        let first_singular = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "getOwnPropertyDescriptor",
        );
        let first_singular_again = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "getOwnPropertyDescriptor",
        );
        let first_plural = property_callable(
            &runtime,
            &mut first,
            first_object.as_object(),
            "getOwnPropertyDescriptors",
        );
        let second_singular = property_callable(
            &runtime,
            &mut second,
            second_object.as_object(),
            "getOwnPropertyDescriptor",
        );
        assert_eq!(first_singular, first_singular_again);
        assert_ne!(first_singular, first_plural);
        assert_ne!(first_singular, second_singular);
        assert_eq!(
            runtime
                .get_prototype_of(first_singular.as_object())
                .unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        drop(second_singular);

        let source = eval_object(
            &mut first,
            r#"(function(){var value=Object();value.x=1;return value})()"#,
        );
        let Value::Object(result) = first
            .call(&first_plural, Value::Undefined, &[Value::Object(source)])
            .unwrap()
        else {
            panic!("Object.getOwnPropertyDescriptors did not return an object");
        };
        let nested = object_property(&runtime, &mut first, &result, "x");
        (first_singular, first_plural, result, nested)
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(singular);
    drop(plural);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(descriptors);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(nested);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}
