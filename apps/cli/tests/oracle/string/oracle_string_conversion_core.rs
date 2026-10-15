use crate::object_graph_observation::{
    define_builtin_data_key as define_data_key, intrinsic_prototype, own_key_names,
};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{property_callable, take_error_object as take_exception_object};
use quickjs_oxide::engine::api::{
    CallableRef, Context, JsString, ObjectRef, PropertyKey, RuntimeError, Value, WellKnownSymbol,
};

#[test]
fn string_conversion_core_cross_realm_and_error_realm_match_quickjs() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_string = first.string_prototype().unwrap();
    let second_string = second.string_prototype().unwrap();
    let first_object = first.object_prototype().unwrap();
    let first_vo = property_callable(&runtime, &mut first, &first_string, "valueOf");
    let second_vo = property_callable(&runtime, &mut second, &second_string, "valueOf");
    let first_ovo = property_callable(&runtime, &mut first, &first_object, "valueOf");
    let first_ots = property_callable(&runtime, &mut first, &first_object, "toString");
    let source = JsString::try_from_utf16([0x41, 0xd800, 0x42]).unwrap();

    let wrapper = box_string(&mut second, &first_ovo, source.clone());
    assert_eq!(
        runtime.get_prototype_of(&wrapper).unwrap(),
        Some(first_string.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        second
            .call(&second_vo, Value::Object(wrapper), &[])
            .unwrap(),
        Value::String(source)
    );

    let tag = PropertyKey::from(
        runtime
            .well_known_symbol(WellKnownSymbol::ToStringTag)
            .expect("well-known symbol"),
    );
    define_data_key(
        &runtime,
        &first_string,
        &tag,
        Value::String(JsString::try_from_utf8("FirstString").unwrap()),
    );
    define_data_key(
        &runtime,
        &second_string,
        &tag,
        Value::String(JsString::try_from_utf8("SecondString").unwrap()),
    );
    assert_eq!(
        second
            .call(
                &first_ots,
                Value::String(JsString::try_from_utf8("x").unwrap()),
                &[]
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("[object FirstString]").unwrap())
    );

    let first_type_error = intrinsic_prototype(&runtime, &mut first, "TypeError");
    let spoof = second.new_object().unwrap();
    assert_eq!(
        second.call(&first_vo, Value::Object(spoof), &[]),
        Err(RuntimeError::Exception)
    );
    let error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(first_type_error)
    );
}

#[test]
fn string_conversion_core_wrapper_realm_graph_is_collectable() {
    let runtime = host_runtime();
    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let object = context.object_prototype().unwrap();
        let ovo = property_callable(&runtime, &mut context, &object, "valueOf");
        box_string(
            &mut context,
            &ovo,
            JsString::try_from_utf16([0x41, 0xd800, 0x42]).unwrap(),
        )
    };
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    assert_eq!(own_key_names(&runtime, &wrapper), ["0", "1", "2", "length"]);
    drop(wrapper);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}

fn box_string(context: &mut Context, ovo: &CallableRef, value: JsString) -> ObjectRef {
    let Value::Object(object) = context.call(ovo, Value::String(value), &[]).unwrap() else {
        panic!("Object.prototype.valueOf did not box String");
    };
    object
}
