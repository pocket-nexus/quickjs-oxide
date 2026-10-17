use crate::object_graph_observation::{
    define_checked_builtin_data as define_data, define_checked_builtin_data_key as define_data_key,
    eval_callable, expect_object, global_callable, global_string as string_global,
};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::core::{
    AccessorValue, CallableRef, Context, DescriptorField, JsString, ObjectRef,
    OrdinaryPropertyDescriptor, PropertyKey, Runtime, RuntimeError, Value, WellKnownSymbol,
};

#[test]
fn number_construct_converts_before_new_target_prototype_and_uses_its_realm() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_global = first.global_object().unwrap();
    let second_global = second.global_object().unwrap();
    let first_number = global_callable(&runtime, &mut first, "Number");
    let second_number = global_callable(&runtime, &mut second, "Number");
    let first_prototype = constructor_prototype(&runtime, &mut first, &first_number);
    let second_prototype = constructor_prototype(&runtime, &mut second, &second_number);
    let first_value_of = property_callable(&runtime, &mut first, &first_prototype, "valueOf");
    let second_value_of = property_callable(&runtime, &mut second, &second_prototype, "valueOf");

    let default_wrapper = expect_object(
        first
            .construct(&first_number, &[Value::Float(-0.0)])
            .unwrap(),
        "default Number construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&default_wrapper).unwrap(),
        Some(first_prototype.try_clone().expect("duplicate root"))
    );
    assert!(
        runtime
            .own_property_keys(&default_wrapper)
            .unwrap()
            .is_empty()
    );
    assert_number(
        first
            .call(&first_value_of, Value::Object(default_wrapper), &[])
            .unwrap(),
        -0.0,
    );

    define_global(
        &runtime,
        &first_global,
        "orderLog",
        Value::String(JsString::try_from_utf8("").unwrap()),
    );
    let custom_prototype = second.new_object().unwrap();
    define_global(
        &runtime,
        &first_global,
        "customNumberPrototype",
        Value::Object(custom_prototype.try_clone().expect("duplicate root")),
    );

    let conversion = eval_callable(
        &runtime,
        &mut first,
        "(function(hint) { orderLog += 'convert:' + hint + '|'; return 7; })",
    );
    let argument = first.new_object().unwrap();
    define_data_key(
        &runtime,
        &argument,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(conversion.as_object().try_clone().expect("duplicate root")),
    );

    let custom_target = bound_constructor(&runtime, &mut second);
    let prototype_getter = eval_callable(
        &runtime,
        &mut first,
        "(function() { orderLog += 'prototype|'; return customNumberPrototype; })",
    );
    define_getter(
        &runtime,
        custom_target.as_object(),
        "prototype",
        prototype_getter,
    );
    let custom_wrapper = expect_object(
        first
            .construct_with_new_target(
                &first_number,
                &custom_target,
                &[Value::Object(argument.try_clone().expect("duplicate root"))],
            )
            .unwrap(),
        "custom-newTarget Number construction",
    );
    assert_eq!(
        string_global(&runtime, &mut first, &first_global, "orderLog"),
        "convert:number|prototype|"
    );
    assert_eq!(
        runtime.get_prototype_of(&custom_wrapper).unwrap(),
        Some(custom_prototype)
    );
    assert_number(
        first
            .call(
                &second_value_of,
                Value::Object(custom_wrapper.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        7.0,
    );

    // A non-object prototype falls back to the callable newTarget's realm,
    // not the Number constructor's defining or caller realm.
    let fallback_target = bound_constructor(&runtime, &mut second);
    define_data(
        &runtime,
        fallback_target.as_object(),
        "prototype",
        Value::Int(1),
    );
    let fallback_wrapper = expect_object(
        first
            .construct_with_new_target(&first_number, &fallback_target, &[Value::Int(9)])
            .unwrap(),
        "fallback-newTarget Number construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&fallback_wrapper).unwrap(),
        Some(second_prototype.try_clone().expect("duplicate root"))
    );
    assert_number(
        first
            .call(
                &first_value_of,
                Value::Object(fallback_wrapper.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        9.0,
    );

    // Using the other realm's Number itself as newTarget selects its explicit
    // prototype. Number branding is class-based and works with either realm's
    // valueOf method.
    let cross_wrapper = expect_object(
        first
            .construct_with_new_target(&first_number, &second_number, &[Value::Int(11)])
            .unwrap(),
        "cross-realm Number construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&cross_wrapper).unwrap(),
        Some(second_prototype)
    );
    assert_number(
        first
            .call(
                &first_value_of,
                Value::Object(cross_wrapper.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        11.0,
    );
    assert_number(
        second
            .call(&second_value_of, Value::Object(cross_wrapper), &[])
            .unwrap(),
        11.0,
    );

    // Keep the second global live until all realm-dependent assertions above
    // have completed; this also makes an accidental caller-realm fallback
    // distinguishable from the newTarget-realm result.
    drop(second_global);
}

#[test]
fn number_construct_preserves_conversion_and_prototype_getter_throws_in_order() {
    let runtime = host_runtime();
    let mut constructor_context = runtime.new_context().expect("create context");
    let mut target_context = runtime.new_context().expect("create context");
    let global = constructor_context.global_object().unwrap();
    let number = global_callable(&runtime, &mut constructor_context, "Number");
    define_global(
        &runtime,
        &global,
        "orderLog",
        Value::String(JsString::try_from_utf8("").unwrap()),
    );

    let conversion_throw = eval_callable(
        &runtime,
        &mut constructor_context,
        "(function() { orderLog += 'convert-throw|'; throw 71; })",
    );
    let throwing_argument = constructor_context.new_object().unwrap();
    define_data_key(
        &runtime,
        &throwing_argument,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(
            conversion_throw
                .as_object()
                .try_clone()
                .expect("duplicate root"),
        ),
    );
    let target = bound_constructor(&runtime, &mut target_context);
    let getter = eval_callable(
        &runtime,
        &mut constructor_context,
        "(function() { orderLog += 'prototype-should-not-run|'; return null; })",
    );
    define_getter(&runtime, target.as_object(), "prototype", getter);
    assert_eq!(
        constructor_context.construct_with_new_target(
            &number,
            &target,
            &[Value::Object(throwing_argument)],
        ),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        constructor_context.take_exception().unwrap(),
        Some(Value::Int(71))
    );
    assert_eq!(
        string_global(&runtime, &mut constructor_context, &global, "orderLog"),
        "convert-throw|"
    );

    drop(constructor_context.eval("orderLog = ''").unwrap());
    let conversion = eval_callable(
        &runtime,
        &mut constructor_context,
        "(function(hint) { orderLog += 'convert:' + hint + '|'; return 3; })",
    );
    let argument = constructor_context.new_object().unwrap();
    define_data_key(
        &runtime,
        &argument,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(conversion.as_object().try_clone().expect("duplicate root")),
    );
    let getter_throw_target = bound_constructor(&runtime, &mut target_context);
    let throwing_getter = eval_callable(
        &runtime,
        &mut constructor_context,
        "(function() { orderLog += 'prototype-throw|'; throw 72; })",
    );
    define_getter(
        &runtime,
        getter_throw_target.as_object(),
        "prototype",
        throwing_getter,
    );
    assert_eq!(
        constructor_context.construct_with_new_target(
            &number,
            &getter_throw_target,
            &[Value::Object(argument)],
        ),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        constructor_context.take_exception().unwrap(),
        Some(Value::Int(72))
    );
    assert_eq!(
        string_global(&runtime, &mut constructor_context, &global, "orderLog"),
        "convert:number|prototype-throw|"
    );
}

#[test]
fn number_wrapper_keeps_its_realm_graph_alive_until_collection() {
    let runtime = host_runtime();
    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let number = global_callable(&runtime, &mut context, "Number");
        expect_object(
            context.construct(&number, &[Value::Float(-0.0)]).unwrap(),
            "new Number(-0)",
        )
    };

    runtime.run_gc().unwrap();
    drop(wrapper);
    runtime.run_gc().unwrap();
}

fn constructor_prototype(
    runtime: &Runtime,
    context: &mut Context,
    constructor: &CallableRef,
) -> ObjectRef {
    let key = runtime.intern_property_key("prototype").unwrap();
    let Value::Object(prototype) = context.get_property(constructor.as_object(), &key).unwrap()
    else {
        panic!("constructor prototype was not an object");
    };
    prototype
}

fn bound_constructor(runtime: &Runtime, context: &mut Context) -> CallableRef {
    let function = eval_callable(runtime, context, "(function Target() {}).bind(null)");
    assert!(
        runtime.is_constructor(function.as_object()).unwrap(),
        "bound target must remain constructible"
    );
    function
}

fn define_global(runtime: &Runtime, global: &ObjectRef, name: &str, value: Value) {
    define_data(runtime, global, name, value);
}

fn define_getter(runtime: &Runtime, object: &ObjectRef, name: &str, getter: CallableRef) {
    let key = runtime.intern_property_key(name).unwrap();
    assert!(
        runtime
            .define_own_property(
                object,
                &key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter)),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap(),
        "host accessor-property definition was rejected"
    );
}

fn assert_number(value: Value, expected: f64) {
    let actual = value
        .as_number()
        .unwrap_or_else(|| panic!("expected Number, got {value:?}"));
    assert_eq!(actual.to_bits(), expected.to_bits());
}
