use crate::object_graph_observation::require_object as expect_object;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable,
    string_property_with_read_context as string_property, take_exception_object,
};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{Context, JsString, ObjectRef, Runtime, RuntimeError, Value};

// Pins the complete QuickJS 2026-06-04 Reflect intrinsic. In particular, the
// construct-order vectors intentionally follow `js_reflect_construct` rather
// than deriving an order from the prose specification: an explicit newTarget
// is checked first, the array-like argument list is then materialized, and the
// target constructor check happens in `JS_CallConstructor2` afterwards.

const METHODS: &[(&str, usize)] = &[
    ("apply", 3),
    ("construct", 2),
    ("defineProperty", 3),
    ("deleteProperty", 2),
    ("get", 2),
    ("getOwnPropertyDescriptor", 2),
    ("getPrototypeOf", 1),
    ("has", 2),
    ("isExtensible", 1),
    ("ownKeys", 1),
    ("preventExtensions", 1),
    ("set", 3),
    ("setPrototypeOf", 2),
];

#[test]
fn reflect_exposes_all_thirteen_nonconstructable_methods() {
    let (runtime, mut context) = host_runtime_context();
    let reflect = reflect_object(&runtime, &mut context);
    for &(name, expected_length) in METHODS {
        let callable = property_callable(&runtime, &mut context, &reflect, name);
        assert_eq!(
            number_property(&runtime, &mut context, callable.as_object(), "length"),
            expected_length as f64,
            "Reflect.{name}.length drifted",
        );
        assert_eq!(
            string_property(&runtime, &mut context, callable.as_object(), "name"),
            name,
            "Reflect.{name}.name drifted",
        );
        assert!(
            !runtime.is_constructor(callable.as_object()).unwrap(),
            "Reflect.{name} unexpectedly carried the constructor bit",
        );
        assert_eq!(
            context.construct(&callable, &[]),
            Err(RuntimeError::Exception)
        );
        take_exception_object(&mut context, &format!("new Reflect.{name}"));
    }
}

#[test]
fn reflect_cross_realm_results_native_errors_and_user_errors_use_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let reflect = reflect_object(&runtime, &mut defining);
    let descriptor_method = property_callable(
        &runtime,
        &mut defining,
        &reflect,
        "getOwnPropertyDescriptor",
    );
    let own_keys = property_callable(&runtime, &mut defining, &reflect, "ownKeys");
    let get = property_callable(&runtime, &mut defining, &reflect, "get");

    let defining_object_prototype = defining.object_prototype().unwrap();
    let defining_array_prototype = eval_object(&mut defining, "Array.prototype", "Array prototype");
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
    assert_ne!(defining_type_error, caller_type_error);

    let target = eval_object(&mut caller, "({answer:42})", "caller target");
    let descriptor = expect_object(
        caller
            .call(
                &descriptor_method,
                Value::Undefined,
                &[
                    Value::Object(target.try_clone().expect("duplicate root")),
                    Value::String(JsString::try_from_utf8("answer").unwrap()),
                ],
            )
            .unwrap(),
        "cross-realm Reflect.getOwnPropertyDescriptor result",
    );
    assert_eq!(
        runtime.get_prototype_of(&descriptor).unwrap(),
        Some(defining_object_prototype),
        "descriptor object used the calling realm",
    );

    let keys = expect_object(
        caller
            .call(
                &own_keys,
                Value::Undefined,
                &[Value::Object(target.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        "cross-realm Reflect.ownKeys result",
    );
    assert_eq!(
        runtime.get_prototype_of(&keys).unwrap(),
        Some(defining_array_prototype),
        "ownKeys Array used the calling realm",
    );

    assert_eq!(
        caller.call(
            &get,
            Value::Undefined,
            &[
                Value::Int(1),
                Value::String(JsString::try_from_utf8("x").unwrap()),
            ],
        ),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "cross-realm Reflect.get TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "native target validation error used the calling realm",
    );

    let throwing_key = eval_object(
        &mut caller,
        r#"(function(){
            var key=Object();
            key[Symbol.toPrimitive]=function(){throw new TypeError("caller key")};
            return key;
        })()"#,
        "caller throwing property key",
    );
    assert_eq!(
        caller.call(
            &get,
            Value::Undefined,
            &[Value::Object(target), Value::Object(throwing_key)],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller property-key TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error.try_clone().expect("duplicate root")),
        "Reflect.get replaced a user-thrown caller-realm error",
    );

    assert_eq!(caller.construct(&get, &[]), Err(RuntimeError::Exception));
    let constructor_error = take_exception_object(&mut caller, "new foreign Reflect.get");
    assert_eq!(
        runtime.get_prototype_of(&constructor_error).unwrap(),
        Some(caller_type_error),
        "generic non-constructor rejection did not use the caller realm",
    );
}

fn reflect_object(runtime: &Runtime, context: &mut Context) -> ObjectRef {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key("Reflect").unwrap();
    let Value::Object(reflect) = context.get_property(&global, &key).unwrap() else {
        panic!("global Reflect was not an object");
    };
    reflect
}

fn number_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> f64 {
    let key = runtime.intern_property_key(name).unwrap();
    context
        .get_property(object, &key)
        .unwrap_or_else(|error| panic!("read numeric property {name}: {error}"))
        .as_number()
        .unwrap_or_else(|| panic!("{name} was not numeric"))
}
