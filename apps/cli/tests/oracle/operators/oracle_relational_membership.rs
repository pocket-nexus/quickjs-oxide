use crate::object_graph_observation::{function, get_property};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_error_object as take_exception_object;
use quickjs_oxide::engine::api::{
    Context, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor, PropertyKey, Runtime,
    RuntimeError, Value, WellKnownSymbol,
};

#[test]
fn membership_errors_follow_opcode_conversion_and_method_defining_realms() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_type_error = intrinsic_prototype(&runtime, &mut first, "TypeError");
    let second_type_error = intrinsic_prototype(&runtime, &mut second, "TypeError");
    assert_ne!(first_type_error, second_type_error);

    for source in [
        "(function(){ return 'x' in 1; })",
        "(function(){ return 1 instanceof 2; })",
    ] {
        let probe = function(&runtime, &mut first, source);
        assert_eq!(
            second.call(&probe, Value::Undefined, &[]),
            Err(RuntimeError::Exception)
        );
        let error = take_exception_object(&mut second);
        assert_eq!(
            runtime.get_prototype_of(&error).unwrap(),
            Some(first_type_error.try_clone().expect("duplicate root")),
            "opcode framework error did not use its bytecode realm for {source:?}"
        );
    }

    let has_instance = PropertyKey::from(
        runtime
            .well_known_symbol(WellKnownSymbol::HasInstance)
            .expect("well-known symbol"),
    );
    let custom_target = second.new_object().unwrap();
    let custom_method = function(
        &runtime,
        &mut first,
        "(function(){ throw new TypeError('custom hasInstance realm'); })",
    );
    define_data_key(
        &mut second,
        &custom_target,
        &has_instance,
        Value::Object(
            custom_method
                .as_object()
                .try_clone()
                .expect("duplicate root"),
        ),
    );
    define_global(
        &runtime,
        &mut second,
        "realmCustomTarget",
        Value::Object(custom_target),
    );
    let custom_probe = function(
        &runtime,
        &mut second,
        "(function(){ return Function instanceof realmCustomTarget; })",
    );
    assert_eq!(
        second.call(&custom_probe, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    );
    let custom_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&custom_error).unwrap(),
        Some(first_type_error.try_clone().expect("duplicate root"))
    );

    let bad_target = function(&runtime, &mut first, "(function RealmBadPrototype(){})");
    let prototype = runtime.intern_property_key("prototype").unwrap();
    assert!(
        first
            .set_property(bad_target.as_object(), &prototype, Value::Int(1))
            .unwrap()
    );
    define_global(
        &runtime,
        &mut second,
        "realmBadPrototype",
        Value::Object(bad_target.as_object().try_clone().expect("duplicate root")),
    );
    let native_probe = function(
        &runtime,
        &mut second,
        "(function(){ return Function instanceof realmBadPrototype; })",
    );
    assert_eq!(
        second.call(&native_probe, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    );
    let native_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(first_type_error.try_clone().expect("duplicate root")),
        "standard hasInstance error did not use the native method realm"
    );

    let null_target = function(&runtime, &mut first, "(function RealmNullHasInstance(){})");
    define_data_key(
        &mut first,
        null_target.as_object(),
        &has_instance,
        Value::Null,
    );
    assert!(
        first
            .set_property(null_target.as_object(), &prototype, Value::Int(1))
            .unwrap()
    );
    define_global(
        &runtime,
        &mut second,
        "realmNullHasInstance",
        Value::Object(null_target.as_object().try_clone().expect("duplicate root")),
    );
    let null_probe = function(
        &runtime,
        &mut second,
        "(function(){ return Function instanceof realmNullHasInstance; })",
    );
    assert_eq!(
        second.call(&null_probe, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    );
    let null_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&null_error).unwrap(),
        Some(second_type_error.try_clone().expect("duplicate root")),
        "nullish hasInstance fallback did not use the opcode realm"
    );

    let to_primitive = PropertyKey::from(
        runtime
            .well_known_symbol(WellKnownSymbol::ToPrimitive)
            .expect("well-known symbol"),
    );
    let throwing_key = second.new_object().unwrap();
    let throwing_conversion = function(
        &runtime,
        &mut first,
        "(function(){ throw new TypeError('key conversion realm'); })",
    );
    define_data_key(
        &mut second,
        &throwing_key,
        &to_primitive,
        Value::Object(
            throwing_conversion
                .as_object()
                .try_clone()
                .expect("duplicate root"),
        ),
    );
    define_global(
        &runtime,
        &mut second,
        "realmThrowingKey",
        Value::Object(throwing_key),
    );
    let target = second.new_object().unwrap();
    define_global(
        &runtime,
        &mut second,
        "realmMembershipTarget",
        Value::Object(target),
    );
    let throwing_key_probe = function(
        &runtime,
        &mut second,
        "(function(){ return realmThrowingKey in realmMembershipTarget; })",
    );
    assert_eq!(
        second.call(&throwing_key_probe, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    );
    let conversion_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&conversion_error).unwrap(),
        Some(first_type_error)
    );

    let invalid_key = second.new_object().unwrap();
    let invalid_conversion = function(&runtime, &mut first, "(function(){ return Function; })");
    define_data_key(
        &mut second,
        &invalid_key,
        &to_primitive,
        Value::Object(
            invalid_conversion
                .as_object()
                .try_clone()
                .expect("duplicate root"),
        ),
    );
    define_global(
        &runtime,
        &mut second,
        "realmInvalidKey",
        Value::Object(invalid_key),
    );
    let invalid_key_probe = function(
        &runtime,
        &mut second,
        "(function(){ return realmInvalidKey in realmMembershipTarget; })",
    );
    assert_eq!(
        second.call(&invalid_key_probe, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    );
    let invalid_conversion_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&invalid_conversion_error).unwrap(),
        Some(second_type_error),
        "post-call ToPrimitive framework error did not return to the opcode realm"
    );
}

fn intrinsic_prototype(runtime: &Runtime, context: &mut Context, name: &str) -> ObjectRef {
    let global = context.global_object().unwrap();
    let Value::Object(constructor) = get_property(runtime, context, &global, name) else {
        panic!("{name} constructor was not an object");
    };
    let Value::Object(prototype) = get_property(runtime, context, &constructor, "prototype") else {
        panic!("{name}.prototype was not an object");
    };
    prototype
}

fn define_global(runtime: &Runtime, context: &mut Context, name: &str, value: Value) {
    let global = context.global_object().unwrap();
    define_data_name(runtime, context, &global, name, value);
}

fn define_data_name(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
    value: Value,
) {
    let key = runtime.intern_property_key(name).unwrap();
    define_data_key(context, object, &key, value);
}

fn define_data_key(context: &mut Context, object: &ObjectRef, key: &PropertyKey, value: Value) {
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
