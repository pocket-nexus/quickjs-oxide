use crate::oracle_cases::GroupedCase as Case;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::core::DescriptorField;
use quickjs_oxide::engine::api::core::OrdinaryPropertyDescriptor;
use quickjs_oxide::engine::api::core::WellKnownSymbol;
use quickjs_oxide::engine::api::core::{
    CallableRef, CompleteOrdinaryPropertyDescriptor, Context, ObjectRef, PropertyKey, Runtime,
    Value,
};

pub(crate) fn global_callable(runtime: &Runtime, context: &mut Context, name: &str) -> CallableRef {
    let global = context.global_object().unwrap();
    property_callable(runtime, context, &global, name)
}

pub(crate) fn intrinsic_prototype(
    runtime: &Runtime,
    context: &mut Context,
    constructor_name: &str,
) -> ObjectRef {
    let constructor = global_callable(runtime, context, constructor_name);
    let Value::Object(prototype) = context
        .get_property(
            constructor.as_object(),
            &runtime.intern_property_key("prototype").unwrap(),
        )
        .unwrap()
    else {
        panic!("{constructor_name}.prototype was not an object");
    };
    prototype
}

pub(crate) fn eval_object(context: &mut Context, source: &str) -> ObjectRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("{source:?} did not evaluate to an object");
    };
    object
}

pub(crate) fn own_key_names(runtime: &Runtime, object: &ObjectRef) -> Vec<String> {
    runtime
        .own_property_keys(object)
        .unwrap()
        .into_iter()
        .map(|key| {
            runtime
                .property_key_to_js_string(&key)
                .unwrap()
                .to_utf8_lossy()
        })
        .collect()
}

pub(crate) fn data_descriptor(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
) -> (Value, bool, bool, bool) {
    let CompleteOrdinaryPropertyDescriptor::Data {
        value,
        writable,
        enumerable,
        configurable,
    } = runtime
        .get_own_property(object, key)
        .unwrap()
        .expect("missing data descriptor")
    else {
        panic!("property was not a data descriptor");
    };
    (value, writable, enumerable, configurable)
}

pub(crate) fn int_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> i32 {
    let Value::Int(value) = context
        .get_property(object, &runtime.intern_property_key(name).unwrap())
        .unwrap()
    else {
        panic!("{name} was not an Int property");
    };
    value
}

// Helpers shared verbatim by several oracle modules.

pub(crate) fn expect_object(value: Value, description: &str) -> ObjectRef {
    let Value::Object(object) = value else {
        panic!("{description} did not produce an object");
    };
    object
}

pub(crate) fn function(runtime: &Runtime, context: &mut Context, source: &str) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("function probe did not return an object: {source}");
    };
    runtime
        .as_callable(&object)
        .unwrap()
        .unwrap_or_else(|| panic!("function probe was not callable: {source}"))
}

pub(crate) fn define_checked_builtin_data(
    runtime: &Runtime,
    object: &ObjectRef,
    name: &str,
    value: Value,
) {
    let key = runtime.intern_property_key(name).unwrap();
    define_checked_builtin_data_key(runtime, object, &key, value);
}

pub(crate) fn define_checked_builtin_data_key(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
    value: Value,
) {
    assert!(
        runtime
            .define_own_property(
                object,
                key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap(),
        "host data-property definition was rejected"
    );
}

pub(crate) fn eval_callable(runtime: &Runtime, context: &mut Context, source: &str) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("callable source did not produce an object: {source:?}");
    };
    runtime
        .as_callable(&object)
        .unwrap()
        .unwrap_or_else(|| panic!("source did not produce a callable: {source:?}"))
}

pub(crate) fn global_string(
    runtime: &Runtime,
    context: &mut Context,
    global: &ObjectRef,
    name: &str,
) -> String {
    let Value::String(value) = context
        .get_property(global, &runtime.intern_property_key(name).unwrap())
        .unwrap()
    else {
        panic!("global {name} was not a string");
    };
    value.to_utf8_lossy()
}

pub(crate) fn error_string_property(
    runtime: &Runtime,
    context: &mut Context,
    error: &ObjectRef,
    name: &str,
    case: &Case,
) -> String {
    let key = runtime
        .intern_property_key(name)
        .unwrap_or_else(|failure| panic!("intern Error.{name} key: {failure}"));
    let Value::String(value) = context.get_property(error, &key).unwrap_or_else(|failure| {
        panic!(
            "read Error.{name} for {} / {}: {failure}",
            case.group, case.description,
        )
    }) else {
        panic!(
            "Error.{name} was not a string for {} / {}",
            case.group, case.description,
        );
    };
    value.to_utf8_lossy()
}

pub(crate) fn object_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> ObjectRef {
    let Value::Object(value) = context
        .get_property(object, &runtime.intern_property_key(name).unwrap())
        .unwrap()
    else {
        panic!("{name} was not an object property");
    };
    value
}

pub(crate) fn eval_source_callable(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("{source:?} did not evaluate to an object");
    };
    runtime
        .as_callable(&object)
        .unwrap()
        .unwrap_or_else(|| panic!("{source:?} was not callable"))
}

pub(crate) fn callable_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> CallableRef {
    let Value::Object(value) = checked_property_value(runtime, context, object, name) else {
        panic!("{name} was not an object");
    };
    runtime
        .as_callable(&value)
        .unwrap()
        .unwrap_or_else(|| panic!("{name} was not callable"))
}

pub(crate) fn checked_property_value(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> Value {
    let key = runtime.intern_property_key(name).unwrap();
    context
        .get_property(object, &key)
        .unwrap_or_else(|error| panic!("read property {name}: {error}"))
}

pub(crate) fn define_named_builtin_data(
    runtime: &Runtime,
    object: &ObjectRef,
    name: &str,
    value: Value,
) {
    let key = runtime.intern_property_key(name).unwrap();
    define_builtin_data_key(runtime, object, &key, value);
}

pub(crate) fn define_builtin_data_key(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
    value: Value,
) {
    assert!(
        runtime
            .define_own_property(
                object,
                key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

pub(crate) fn define_to_primitive(runtime: &Runtime, object: &ObjectRef, callable: CallableRef) {
    define_builtin_data_key(
        runtime,
        object,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(callable.as_object().try_clone().expect("duplicate root")),
    );
}
