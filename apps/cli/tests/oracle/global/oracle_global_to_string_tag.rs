use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::{
    DescriptorField, JsString, OrdinaryPropertyDescriptor, PropertyKey, Value, WellKnownSymbol,
};

#[test]
fn object_to_string_cross_realm_uses_receiver_tag_and_method_defining_realm() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_global = first.global_object().unwrap();
    let second_global = second.global_object().unwrap();
    let first_object_prototype = first.object_prototype().unwrap();
    let second_object_prototype = second.object_prototype().unwrap();
    let first_to_string =
        property_callable(&runtime, &mut first, &first_object_prototype, "toString");
    let second_to_string =
        property_callable(&runtime, &mut second, &second_object_prototype, "toString");
    let tag = PropertyKey::from(
        runtime
            .well_known_symbol(WellKnownSymbol::ToStringTag)
            .expect("well-known symbol"),
    );

    define_data_key(
        &mut first,
        &first_global,
        &tag,
        Value::String(JsString::try_from_utf8("first-global").unwrap()),
        false,
        false,
        true,
    );
    define_data_key(
        &mut second,
        &second_global,
        &tag,
        Value::String(JsString::try_from_utf8("second-global").unwrap()),
        false,
        false,
        true,
    );
    assert_eq!(
        second
            .call(
                &first_to_string,
                Value::Object(second_global.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("[object second-global]").unwrap()),
        "a foreign native method must read the receiver global's own tag"
    );
    assert_eq!(
        first
            .call(
                &second_to_string,
                Value::Object(first_global.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("[object first-global]").unwrap())
    );

    let first_number_prototype = first.number_prototype().unwrap();
    let second_number_prototype = second.number_prototype().unwrap();
    define_data_key(
        &mut first,
        &first_number_prototype,
        &tag,
        Value::String(JsString::try_from_utf8("FirstNumber").unwrap()),
        false,
        false,
        true,
    );
    define_data_key(
        &mut second,
        &second_number_prototype,
        &tag,
        Value::String(JsString::try_from_utf8("SecondNumber").unwrap()),
        false,
        false,
        true,
    );
    assert_eq!(
        second.call(&first_to_string, Value::Int(1), &[]).unwrap(),
        Value::String(JsString::try_from_utf8("[object FirstNumber]").unwrap()),
        "primitive boxing must use Object.prototype.toString's defining realm"
    );
    assert_eq!(
        first.call(&second_to_string, Value::Int(1), &[]).unwrap(),
        Value::String(JsString::try_from_utf8("[object SecondNumber]").unwrap())
    );
}

fn define_data_key(
    context: &mut quickjs_oxide::engine::api::Context,
    object: &quickjs_oxide::engine::api::ObjectRef,
    key: &PropertyKey,
    value: Value,
    writable: bool,
    enumerable: bool,
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
                    enumerable: DescriptorField::Present(enumerable),
                    configurable: DescriptorField::Present(configurable),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}
