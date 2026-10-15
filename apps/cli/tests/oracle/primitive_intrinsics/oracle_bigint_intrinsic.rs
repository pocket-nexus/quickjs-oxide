use crate::object_graph_observation::{
    define_data_key_with_flags as define_data_key, define_data_with_flags as define_data,
    expect_object, intrinsic_prototype,
};
use crate::primitive_probe::eval_callable;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use crate::runtime_observation::take_error_object as take_exception_object;
use quickjs_oxide::engine::api::{
    JsBigInt, JsString, PropertyKey, RuntimeError, Value, WellKnownSymbol,
};

#[test]
fn bigint_cross_realm_routes_boxing_lookups_and_native_errors_to_the_defining_realm() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_global = first.global_object().unwrap();
    let second_global = second.global_object().unwrap();
    let first_prototype = first.bigint_prototype().unwrap();
    let second_prototype = second.bigint_prototype().unwrap();
    assert_ne!(first_prototype, second_prototype);

    let first_bigint = property_callable(&runtime, &mut first, &first_global, "BigInt");
    let first_value_of = property_callable(&runtime, &mut first, &first_prototype, "valueOf");
    let second_value_of = property_callable(&runtime, &mut second, &second_prototype, "valueOf");
    let first_object_prototype = first.object_prototype().unwrap();
    let second_object_prototype = second.object_prototype().unwrap();
    let first_object_value_of =
        property_callable(&runtime, &mut first, &first_object_prototype, "valueOf");
    let second_object_value_of =
        property_callable(&runtime, &mut second, &second_object_prototype, "valueOf");
    let first_object_to_string =
        property_callable(&runtime, &mut first, &first_object_prototype, "toString");

    let first_wrapper = expect_object(
        second
            .call(
                &first_object_value_of,
                Value::BigInt(JsBigInt::from(7)),
                &[],
            )
            .unwrap(),
        "foreign Object.prototype.valueOf BigInt boxing",
    );
    let second_wrapper = expect_object(
        first
            .call(
                &second_object_value_of,
                Value::BigInt(JsBigInt::from(9)),
                &[],
            )
            .unwrap(),
        "second-realm Object.prototype.valueOf BigInt boxing",
    );
    assert_eq!(
        runtime.get_prototype_of(&first_wrapper).unwrap(),
        Some(first_prototype.try_clone().expect("duplicate root")),
        "primitive boxing must use the native method's defining realm"
    );
    assert_eq!(
        runtime.get_prototype_of(&second_wrapper).unwrap(),
        Some(second_prototype.try_clone().expect("duplicate root"))
    );
    for (method, wrapper, expected) in [
        (&first_value_of, &first_wrapper, 7),
        (&second_value_of, &first_wrapper, 7),
        (&first_value_of, &second_wrapper, 9),
        (&second_value_of, &second_wrapper, 9),
    ] {
        assert_eq!(
            second
                .call(
                    method,
                    Value::Object(wrapper.try_clone().expect("duplicate root")),
                    &[]
                )
                .unwrap(),
            Value::BigInt(JsBigInt::from(expected)),
            "BigInt wrapper branding must be realm-independent"
        );
    }

    define_data(
        &runtime,
        &first_prototype,
        "__realmMarker",
        Value::String(JsString::try_from_utf8("first").unwrap()),
        true,
        false,
        true,
    );
    define_data(
        &runtime,
        &second_prototype,
        "__realmMarker",
        Value::String(JsString::try_from_utf8("second").unwrap()),
        true,
        false,
        true,
    );
    let first_reader = eval_callable(
        &runtime,
        &mut first,
        "(function() { return (1n).__realmMarker; })",
    );
    assert_eq!(
        second.call(&first_reader, Value::Undefined, &[]).unwrap(),
        Value::String(JsString::try_from_utf8("first").unwrap()),
        "primitive member lookup must use the bytecode function's realm"
    );

    let tag = PropertyKey::from(
        runtime
            .well_known_symbol(WellKnownSymbol::ToStringTag)
            .expect("well-known symbol"),
    );
    define_data_key(
        &runtime,
        &first_prototype,
        &tag,
        Value::String(JsString::try_from_utf8("FirstBigInt").unwrap()),
        false,
        false,
        true,
    );
    define_data_key(
        &runtime,
        &second_prototype,
        &tag,
        Value::String(JsString::try_from_utf8("SecondBigInt").unwrap()),
        false,
        false,
        true,
    );
    assert_eq!(
        second
            .call(&first_object_to_string, Value::BigInt(JsBigInt::one()), &[],)
            .unwrap(),
        Value::String(JsString::try_from_utf8("[object FirstBigInt]").unwrap()),
        "Object.prototype.toString must box a primitive in its defining realm"
    );

    let first_type_error = intrinsic_prototype(&runtime, &mut first, "TypeError");
    let second_type_error = intrinsic_prototype(&runtime, &mut second, "TypeError");
    let first_range_error = intrinsic_prototype(&runtime, &mut first, "RangeError");
    assert_ne!(first_type_error, second_type_error);
    let symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("bigint").unwrap()))
        .unwrap();
    assert_eq!(
        second.call(&first_bigint, Value::Undefined, &[Value::Symbol(symbol)],),
        Err(RuntimeError::Exception)
    );
    let conversion_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&conversion_error).unwrap(),
        Some(first_type_error.try_clone().expect("duplicate root")),
        "BigInt conversion errors must use the constructor's defining realm"
    );

    assert_eq!(
        second.call(&first_bigint, Value::Undefined, &[Value::Float(1.5)],),
        Err(RuntimeError::Exception)
    );
    let range_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&range_error).unwrap(),
        Some(first_range_error)
    );

    let spoof = second.new_object().unwrap();
    assert_eq!(
        second.call(&first_value_of, Value::Object(spoof), &[]),
        Err(RuntimeError::Exception)
    );
    let brand_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&brand_error).unwrap(),
        Some(first_type_error)
    );

    let user_throw = eval_callable(
        &runtime,
        &mut second,
        "(function() { throw new TypeError('foreign conversion'); })",
    );
    let throwing_input = second.new_object().unwrap();
    define_data_key(
        &runtime,
        &throwing_input,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(user_throw.as_object().try_clone().expect("duplicate root")),
        true,
        false,
        true,
    );
    assert_eq!(
        first.call(
            &first_bigint,
            Value::Undefined,
            &[Value::Object(throwing_input)],
        ),
        Err(RuntimeError::Exception)
    );
    let user_error = take_exception_object(&mut first);
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(second_type_error),
        "an explicit user throw must retain the conversion function's realm"
    );

    drop(second_global);
}

#[test]
fn bigint_wrapper_keeps_its_realm_graph_alive_until_collection() {
    let runtime = host_runtime();
    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let object_prototype = context.object_prototype().unwrap();
        let object_value_of =
            property_callable(&runtime, &mut context, &object_prototype, "valueOf");
        expect_object(
            context
                .call(&object_value_of, Value::BigInt(JsBigInt::from(123)), &[])
                .unwrap(),
            "Object.prototype.valueOf BigInt wrapper",
        )
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1,
        "the live wrapper must retain its prototype and defining context graph"
    );
    drop(wrapper);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").live,
        0,
        "BigInt contexts, prototypes, native functions, and wrappers must be collectable"
    );
}
