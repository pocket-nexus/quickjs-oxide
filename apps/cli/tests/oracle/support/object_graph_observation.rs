use crate::oracle_cases::GroupedCase as Case;
use crate::runtime_observation::property_callable;
use crate::runtime_observation::value_text;
use quickjs_oxide::engine::api::AccessorValue;
use quickjs_oxide::engine::api::DescriptorField;
use quickjs_oxide::engine::api::OrdinaryPropertyDescriptor;
use quickjs_oxide::engine::api::WellKnownSymbol;
use quickjs_oxide::engine::api::{
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

pub(crate) fn data_bits(writable: bool, enumerable: bool, configurable: bool) -> String {
    format!(
        "D:{}{}{}",
        u8::from(writable),
        u8::from(enumerable),
        u8::from(configurable),
    )
}

// Helpers shared verbatim by several oracle modules.

pub(crate) fn object(value: Value) -> ObjectRef {
    let Value::Object(value) = value else {
        panic!("expected an object");
    };
    value
}

pub(crate) fn define_context_global(context: &mut Context, name: &str, value: Value) {
    let runtime = context.runtime().clone();
    let key = runtime.intern_property_key(name).unwrap();
    let global = context.global_object().unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

pub(crate) fn expect_object(value: Value, description: &str) -> ObjectRef {
    let Value::Object(object) = value else {
        panic!("{description} did not produce an object");
    };
    object
}

pub(crate) fn define_global(runtime: &Runtime, context: &mut Context, name: &str, value: Value) {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

pub(crate) fn require_object(value: Value, description: &str) -> ObjectRef {
    let Value::Object(object) = value else {
        panic!("{description} was not an object");
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

pub(crate) fn define_global_via_data(
    runtime: &Runtime,
    context: &mut Context,
    name: &str,
    value: Value,
) {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    define_context_data(runtime, context, &global, &key, value);
}

pub(crate) fn global_value(runtime: &Runtime, context: &mut Context, name: &str) -> Value {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    context.get_property(&global, &key).unwrap()
}

pub(crate) fn define_context_data(
    _runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    key: &PropertyKey,
    value: Value,
) {
    assert!(
        context
            .define_own_property(
                object,
                key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

pub(crate) fn getter_descriptor(getter: CallableRef) -> OrdinaryPropertyDescriptor {
    OrdinaryPropertyDescriptor {
        get: DescriptorField::Present(AccessorValue::Callable(getter)),
        set: DescriptorField::Present(AccessorValue::Undefined),
        enumerable: DescriptorField::Present(true),
        configurable: DescriptorField::Present(true),
        ..OrdinaryPropertyDescriptor::new()
    }
}

pub(crate) fn complete_data_descriptor(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
) -> (Value, bool, bool, bool) {
    let Some(CompleteOrdinaryPropertyDescriptor::Data {
        value,
        writable,
        enumerable,
        configurable,
    }) = runtime.get_own_property(object, key).unwrap()
    else {
        panic!("expected a complete data descriptor");
    };
    (value, writable, enumerable, configurable)
}

pub(crate) const fn bit(value: bool) -> u8 {
    value as u8
}

pub(crate) fn get_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> Value {
    let key = runtime.intern_property_key(name).unwrap();
    context.get_property(object, &key).unwrap()
}

pub(crate) fn property_text(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> String {
    value_text(get_property(runtime, context, object, name))
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

pub(crate) fn own_data_descriptor(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
) -> (Value, bool, bool, bool) {
    let descriptor = runtime
        .get_own_property(object, key)
        .unwrap()
        .expect("expected own property descriptor");
    let CompleteOrdinaryPropertyDescriptor::Data {
        value,
        writable,
        enumerable,
        configurable,
    } = descriptor
    else {
        panic!("expected a data property descriptor");
    };
    (value, writable, enumerable, configurable)
}

pub(crate) fn function_probe(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("function probe did not return an object: {source}");
    };
    runtime.as_callable(&object).unwrap().unwrap()
}

pub(crate) fn define_global_flagged(
    runtime: &Runtime,
    context: &mut Context,
    name: &str,
    value: Value,
) {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    define_context_data_with_flags(context, &global, &key, value, true, true);
}

pub(crate) fn set_global(runtime: &Runtime, context: &mut Context, name: &str, value: Value) {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    assert!(context.set_property(&global, &key, value).unwrap());
}

pub(crate) fn string_global(runtime: &Runtime, context: &mut Context, name: &str) -> String {
    let Value::String(value) = global_value(runtime, context, name) else {
        panic!("global {name} was not a string");
    };
    value.to_utf8_lossy()
}

pub(crate) fn define_context_data_with_flags(
    context: &mut Context,
    object: &ObjectRef,
    key: &PropertyKey,
    value: Value,
    writable: bool,
    configurable: bool,
) {
    assert!(
        context
            .define_own_property(
                object,
                key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(writable),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(configurable),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

pub(crate) fn define_context_accessor(
    context: &mut Context,
    object: &ObjectRef,
    key: &PropertyKey,
    get: AccessorValue,
    set: AccessorValue,
    configurable: bool,
) {
    assert!(
        context
            .define_own_property(
                object,
                key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(get),
                    set: DescriptorField::Present(set),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(configurable),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

pub(crate) fn define_data_with_flags(
    runtime: &Runtime,
    object: &ObjectRef,
    name: &str,
    value: Value,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) {
    let key = runtime.intern_property_key(name).unwrap();
    define_data_key_with_flags(
        runtime,
        object,
        &key,
        value,
        writable,
        enumerable,
        configurable,
    );
}

pub(crate) fn define_data_key_with_flags(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
    value: Value,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) {
    assert!(
        runtime
            .define_own_property(
                object,
                key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(writable),
                    enumerable: DescriptorField::Present(enumerable),
                    configurable: DescriptorField::Present(configurable),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap(),
        "host data-property definition was rejected"
    );
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

pub(crate) fn define_global_data(
    runtime: &Runtime,
    context: &mut Context,
    name: &str,
    value: Value,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key(name).unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(writable),
                    enumerable: DescriptorField::Present(enumerable),
                    configurable: DescriptorField::Present(configurable),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

pub(crate) fn function_setup(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("function setup did not produce an object: {source}");
    };
    runtime.as_callable(&object).unwrap().unwrap()
}

pub(crate) fn context_global_value(context: &mut Context, name: &str) -> Value {
    let runtime = context.runtime();
    let key = runtime.intern_property_key(name).unwrap();
    let global = context.global_object().unwrap();
    context.get_property(&global, &key).unwrap()
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

pub(crate) fn eval_named_callable(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> CallableRef {
    let callable = eval_described_object(context, source, description);
    runtime
        .as_callable(&callable)
        .unwrap()
        .unwrap_or_else(|| panic!("{description} was not callable"))
}

pub(crate) fn eval_described_object(
    context: &mut Context,
    source: &str,
    description: &str,
) -> ObjectRef {
    require_object(
        context
            .eval(source)
            .unwrap_or_else(|error| panic!("evaluate {description}: {error}")),
        description,
    )
}

pub(crate) fn eval_optional_callable(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> Option<CallableRef> {
    let value = context
        .eval(source)
        .unwrap_or_else(|error| panic!("Rust rejected {description} ({source:?}): {error}"));
    let Value::Object(object) = value else {
        return None;
    };
    runtime
        .as_callable(&object)
        .unwrap_or_else(|error| panic!("inspect {description}: {error}"))
}

pub(crate) fn define_builtin_data(runtime: &Runtime, object: &ObjectRef, name: &str, value: Value) {
    assert!(
        runtime
            .define_own_property(
                object,
                &runtime.intern_property_key(name).unwrap(),
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

pub(crate) fn data_property(value: Value) -> OrdinaryPropertyDescriptor {
    OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(value),
        writable: DescriptorField::Present(true),
        enumerable: DescriptorField::Present(true),
        configurable: DescriptorField::Present(true),
        ..OrdinaryPropertyDescriptor::new()
    }
}

pub(crate) fn eval_string_with_global(
    runtime: &Runtime,
    context: &mut Context,
    name: &str,
    value: Value,
    source: &str,
) -> String {
    define_global_with_message(runtime, context, name, value);
    let Value::String(value) = context.eval(source).expect("cross-realm observation") else {
        panic!("cross-realm observation did not return a string");
    };
    value.to_utf8_lossy()
}

pub(crate) fn define_global_with_message(
    runtime: &Runtime,
    context: &mut Context,
    name: &str,
    value: Value,
) {
    let key = runtime.intern_property_key(name).unwrap();
    assert!(
        context
            .define_own_property(
                &context.global_object().unwrap(),
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap(),
        "could not define {name}",
    );
}
