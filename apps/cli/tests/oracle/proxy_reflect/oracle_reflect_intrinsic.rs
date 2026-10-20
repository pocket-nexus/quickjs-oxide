use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::{
    exception_object, property_callable_with_read_context as property_callable,
    string_property_with_read_context as string_property,
};
use quickjs_oxide::engine::api::core::{Context, ObjectRef, Runtime, Value};

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
        exception_object(
            context.construct(&callable, &[]),
            &format!("new Reflect.{name}"),
        );
    }
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
