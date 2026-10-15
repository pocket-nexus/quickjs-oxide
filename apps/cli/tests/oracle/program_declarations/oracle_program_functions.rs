use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_exception_object as take_object_exception;

use crate::object_graph_observation::{define_global_data, function_setup as function};
use crate::runtime_oracle::value_type;
use quickjs_oxide::engine::api::{
    AccessorValue, CallableRef, CompleteOrdinaryPropertyDescriptor, Context, DescriptorField,
    JsString, OrdinaryPropertyDescriptor, Runtime, RuntimeError, Value,
};

#[test]
fn program_function_cross_realm_matches_pinned_c_api() {
    let _oracle = require_qjs_oracle!("Program function cross-realm oracle");

    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    drop(defining.eval("globalThis.realmTag='A'").unwrap());
    drop(caller.eval("globalThis.realmTag='B'").unwrap());

    let bytecode = defining
        .compile("function crossRealmFunction(){return realmTag}crossRealmFunction")
        .unwrap();
    assert_eq!(
        defining.eval("typeof crossRealmFunction").unwrap(),
        Value::String(JsString::try_from_utf8("undefined").unwrap())
    );
    assert_eq!(
        caller.eval("typeof crossRealmFunction").unwrap(),
        Value::String(JsString::try_from_utf8("undefined").unwrap())
    );

    let Value::Object(function_a) = defining.execute(&bytecode).unwrap() else {
        panic!("defining execution did not return its Program function");
    };
    let Value::Object(function_b) = caller.execute(&bytecode).unwrap() else {
        panic!("caller execution did not return its Program function");
    };
    let Value::Object(function_prototype_a) = defining.eval("Function.prototype").unwrap() else {
        panic!("defining Function.prototype was not an object");
    };
    let Value::Object(function_prototype_b) = caller.eval("Function.prototype").unwrap() else {
        panic!("caller Function.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&function_a).unwrap(),
        Some(function_prototype_a.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        runtime.get_prototype_of(&function_b).unwrap(),
        Some(function_prototype_a.try_clone().expect("duplicate root"))
    );
    assert_ne!(
        runtime.get_prototype_of(&function_b).unwrap(),
        Some(function_prototype_b)
    );
    let callable_a = runtime.as_callable(&function_a).unwrap().unwrap();
    let callable_b = runtime.as_callable(&function_b).unwrap().unwrap();
    assert_eq!(
        defining.call(&callable_a, Value::Undefined, &[]).unwrap(),
        Value::String(JsString::try_from_utf8("A").unwrap())
    );
    assert_eq!(
        caller.call(&callable_b, Value::Undefined, &[]).unwrap(),
        Value::String(JsString::try_from_utf8("B").unwrap())
    );
    assert_eq!(
        defining.eval("crossRealmFunction()").unwrap(),
        Value::String(JsString::try_from_utf8("A").unwrap())
    );
    assert_eq!(
        caller.eval("crossRealmFunction()").unwrap(),
        Value::String(JsString::try_from_utf8("B").unwrap())
    );

    define_global_data(
        &runtime,
        &mut caller,
        "blockedRealmFunction",
        Value::Int(1),
        false,
        true,
        false,
    );
    let blocked = defining
        .compile("Function.preflightSide=1;function blockedRealmFunction(){}")
        .unwrap();
    assert_eq!(
        caller.execute(&blocked),
        Err(RuntimeError::Exception),
        "caller preflight unexpectedly accepted a fixed property"
    );
    let error = take_object_exception(&mut caller, "cross-realm function preflight");
    let Value::Object(caller_type_prototype) = caller.eval("TypeError.prototype").unwrap() else {
        panic!("caller TypeError.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_prototype)
    );
    assert_eq!(
        defining.eval("typeof Function.preflightSide").unwrap(),
        Value::String(JsString::try_from_utf8("undefined").unwrap())
    );
    assert_eq!(
        caller.eval("typeof Function.preflightSide").unwrap(),
        Value::String(JsString::try_from_utf8("undefined").unwrap())
    );

    let body_error = defining
        .compile("function realmBodyError(){missingRealmBody}realmBodyError()")
        .unwrap();
    assert_eq!(
        caller.execute(&body_error),
        Err(RuntimeError::Exception),
        "cross-realm function body unexpectedly succeeded"
    );
    let error = take_object_exception(&mut caller, "cross-realm function body");
    let Value::Object(defining_reference_prototype) =
        defining.eval("ReferenceError.prototype").unwrap()
    else {
        panic!("defining ReferenceError.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_reference_prototype)
    );

    drop(
        defining
            .eval("Function.aFunctionGets=0;Function.aFunctionSets=0")
            .unwrap(),
    );
    drop(
        caller
            .eval("Function.bFunctionGets=0;Function.bFunctionSets=0")
            .unwrap(),
    );
    define_global_accessor(
        &runtime,
        &mut defining,
        "accessRealmFunction",
        "(function(){Function.aFunctionGets++;return 'Aaccess'})",
        "(function(){Function.aFunctionSets++})",
        false,
        true,
    );
    define_global_accessor(
        &runtime,
        &mut caller,
        "accessRealmFunction",
        "(function(){Function.bFunctionGets++;return 'Baccess'})",
        "(function(){Function.bFunctionSets++})",
        false,
        true,
    );
    let accessor = defining
        .compile("function accessRealmFunction(){return 33}accessRealmFunction")
        .unwrap();
    let Value::Object(access_function) = caller.execute(&accessor).unwrap() else {
        panic!("accessor replacement did not return a function");
    };
    assert_eq!(
        runtime.get_prototype_of(&access_function).unwrap(),
        Some(function_prototype_a)
    );
    let access_function = runtime.as_callable(&access_function).unwrap().unwrap();
    assert_eq!(
        caller
            .call(&access_function, Value::Undefined, &[])
            .unwrap(),
        Value::Int(33)
    );
    assert_eq!(
        defining
            .eval("accessRealmFunction+'|'+Function.aFunctionGets")
            .unwrap(),
        Value::String(JsString::try_from_utf8("Aaccess|1").unwrap())
    );
    assert_eq!(
        caller
            .eval("accessRealmFunction()+'|'+Function.bFunctionGets+'|'+Function.bFunctionSets")
            .unwrap(),
        Value::String(JsString::try_from_utf8("33|0|0").unwrap())
    );
    assert_eq!(
        descriptor_text(&runtime, &mut caller, "accessRealmFunction"),
        "data|function|true|true|false"
    );
}

fn define_global_accessor(
    runtime: &Runtime,
    context: &mut Context,
    name: &str,
    getter_source: &str,
    setter_source: &str,
    enumerable: bool,
    configurable: bool,
) {
    let getter = function(runtime, context, getter_source);
    let setter = function(runtime, context, setter_source);
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter)),
                    set: DescriptorField::Present(AccessorValue::Callable(setter)),
                    enumerable: DescriptorField::Present(enumerable),
                    configurable: DescriptorField::Present(configurable),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

fn descriptor_text(runtime: &Runtime, context: &mut Context, name: &str) -> String {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    let Some(descriptor) = context.get_own_property(&global, &key).unwrap() else {
        return "missing".to_owned();
    };
    match descriptor {
        CompleteOrdinaryPropertyDescriptor::Data {
            value,
            writable,
            enumerable,
            configurable,
        } => format!(
            "data|{}|{writable}|{enumerable}|{configurable}",
            value_type(runtime, &value),
        ),
        CompleteOrdinaryPropertyDescriptor::Accessor {
            get,
            set,
            enumerable,
            configurable,
        } => format!(
            "accessor|{}|{}|{enumerable}|{configurable}",
            accessor_type(&get),
            accessor_type(&set),
        ),
    }
}

fn accessor_type(value: &Option<CallableRef>) -> &'static str {
    if value.is_some() {
        "function"
    } else {
        "undefined"
    }
}
