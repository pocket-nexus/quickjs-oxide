use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::property_callable_with_read_context as property_callable;
use quickjs_oxide::engine::api::core::{Context, ObjectRef, Runtime, RuntimeError, Value};

const METHODS: &[(&str, usize)] = &[
    ("min", 2),
    ("max", 2),
    ("abs", 1),
    ("floor", 1),
    ("ceil", 1),
    ("round", 1),
    ("sqrt", 1),
    ("acos", 1),
    ("asin", 1),
    ("atan", 1),
    ("atan2", 2),
    ("cos", 1),
    ("exp", 1),
    ("log", 1),
    ("pow", 2),
    ("sin", 1),
    ("tan", 1),
    ("trunc", 1),
    ("sign", 1),
    ("cosh", 1),
    ("sinh", 1),
    ("tanh", 1),
    ("acosh", 1),
    ("asinh", 1),
    ("atanh", 1),
    ("expm1", 1),
    ("log1p", 1),
    ("log2", 1),
    ("log10", 1),
    ("cbrt", 1),
    ("hypot", 2),
    ("random", 0),
    ("f16round", 1),
    ("fround", 1),
    ("imul", 2),
    ("clz32", 1),
    ("sumPrecise", 1),
];

#[test]
fn math_methods_are_not_constructable() {
    let (runtime, mut context) = host_runtime_context();
    let math = math_object(&runtime, &mut context);
    for &(name, _) in METHODS {
        let callable = property_callable(&runtime, &mut context, &math, name);
        assert!(
            !runtime.is_constructor(callable.as_object()).unwrap(),
            "Math.{name} unexpectedly carried the constructor bit",
        );
        assert!(matches!(
            context.construct(&callable, &[]),
            Err(RuntimeError::Exception)
        ));
        drop(
            context
                .take_exception()
                .unwrap_or_else(|error| panic!("take new Math.{name} exception: {error}"))
                .unwrap_or_else(|| panic!("new Math.{name} did not publish an exception")),
        );
    }
}

fn math_object(runtime: &Runtime, context: &mut Context) -> ObjectRef {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key("Math").unwrap();
    let Value::Object(math) = context.get_property(&global, &key).unwrap() else {
        panic!("global Math was not an object");
    };
    math
}
