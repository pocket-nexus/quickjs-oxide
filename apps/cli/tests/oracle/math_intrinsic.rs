use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{Context, ObjectRef, Runtime, RuntimeError, Value};

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

#[test]
fn math_native_errors_use_the_method_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
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

    let math = math_object(&runtime, &mut defining);
    let abs = property_callable(&runtime, &mut defining, &math, "abs");
    let sum_precise = property_callable(&runtime, &mut defining, &math, "sumPrecise");

    let symbol = runtime.new_symbol(None).unwrap();
    assert!(matches!(
        caller.call(&abs, Value::Undefined, &[Value::Symbol(symbol)]),
        Err(RuntimeError::Exception),
    ));
    let native_conversion = take_exception_object(&mut caller, "cross-realm Math.abs TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_conversion).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "Math.abs allocated its ToNumber TypeError in the calling realm",
    );

    let invalid_items = eval_object(&mut caller, "[1,'2']", "caller invalid Number iterable");
    assert!(matches!(
        caller.call(
            &sum_precise,
            Value::Undefined,
            &[Value::Object(invalid_items)],
        ),
        Err(RuntimeError::Exception),
    ));
    let native_item = take_exception_object(&mut caller, "cross-realm sumPrecise TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_item).unwrap(),
        Some(defining_type_error),
        "Math.sumPrecise allocated its item TypeError in the calling realm",
    );

    let throwing_argument = eval_object(
        &mut caller,
        "(function(){var value=Object();value.valueOf=function(){throw new TypeError('caller')};return value})()",
        "caller throwing numeric argument",
    );
    assert!(matches!(
        caller.call(&abs, Value::Undefined, &[Value::Object(throwing_argument)],),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "caller conversion TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Math.abs replaced an argument conversion throw with a native error",
    );
}

fn math_object(runtime: &Runtime, context: &mut Context) -> ObjectRef {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key("Math").unwrap();
    let Value::Object(math) = context.get_property(&global, &key).unwrap() else {
        panic!("global Math was not an object");
    };
    math
}
