use crate::object_graph_observation::function;
use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::show_exception;
use quickjs_oxide::engine::api::{
    CallableRef, CompleteOrdinaryPropertyDescriptor, Context, ObjectRef, PropertyKey, Runtime,
    RuntimeError, Value,
};

#[test]
fn generator_callables_do_not_receive_the_ordinary_sloppy_restricted_property_exception() {
    let (runtime, mut context) = host_runtime_context();
    let function_prototype = context.function_prototype().unwrap();
    let restricted = ["caller", "arguments"].map(|name| {
        let key = runtime.intern_property_key(name).unwrap();
        let (get, set, _, _) = accessor_descriptor(&runtime, &function_prototype, &key);
        (name, key, get, set)
    });
    let generators = [
        (
            "syntax",
            function(&runtime, &mut context, "(function* syntaxGenerator(){})"),
        ),
        (
            "dynamic",
            function(
                &runtime,
                &mut context,
                "new (Object.getPrototypeOf(function*(){}).constructor)()",
            ),
        ),
    ];

    for (generator_kind, generator) in generators {
        for (property_name, key, get, set) in &restricted {
            assert!(
                !runtime
                    .has_own_property(generator.as_object(), key)
                    .unwrap(),
                "{generator_kind} generator unexpectedly had an own {property_name} property"
            );
            assert_eq!(
                observe_call(
                    &runtime,
                    &mut context,
                    get,
                    Value::Object(generator.as_object().try_clone().expect("duplicate root")),
                    &[],
                ),
                "throw:TypeError|invalid property access",
                "{generator_kind} generator {property_name} getter did not use the poison accessor"
            );
            assert_eq!(
                observe_call(
                    &runtime,
                    &mut context,
                    set,
                    Value::Object(generator.as_object().try_clone().expect("duplicate root")),
                    &[Value::Int(1)],
                ),
                "throw:TypeError|invalid property access",
                "{generator_kind} generator {property_name} setter did not use the poison accessor"
            );
        }
    }
}

fn accessor_descriptor(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
) -> (CallableRef, CallableRef, bool, bool) {
    let Some(CompleteOrdinaryPropertyDescriptor::Accessor {
        get: Some(get),
        set: Some(set),
        enumerable,
        configurable,
    }) = runtime.get_own_property(object, key).unwrap()
    else {
        panic!("expected a complete accessor descriptor");
    };
    (get, set, enumerable, configurable)
}

fn observe_call(
    runtime: &Runtime,
    context: &mut Context,
    callable: &CallableRef,
    this_value: Value,
    arguments: &[Value],
) -> String {
    match context.call(callable, this_value, arguments) {
        Ok(value) => show_value(value),
        Err(RuntimeError::Exception) => {
            let exception = context
                .take_exception()
                .unwrap()
                .expect("exception completion had no value");
            show_exception(runtime, context, exception)
        }
        Err(error) => panic!("call probe failed with an engine error: {error}"),
    }
}

fn show_value(value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::String(value) => format!("string:{}", value.to_utf8_lossy()),
        value => panic!("unexpected probe return value: {value:?}"),
    }
}
