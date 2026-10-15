use crate::object_graph_observation::intrinsic_prototype;
use crate::object_graph_observation::{
    define_data_key_with_flags as define_data_key, expect_object,
};
use crate::primitive_probe::eval_callable;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use crate::runtime_observation::take_error_object as take_exception_object;
use quickjs_oxide::engine::api::{
    CallableRef, Context, JsString, ObjectRef, PropertyKey, Runtime, RuntimeError, SymbolRef,
    Value, WellKnownSymbol,
};

#[test]
fn symbol_cross_realm_routes_registry_boxing_lookup_and_errors() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_global = first.global_object().unwrap();
    let second_global = second.global_object().unwrap();
    let first_symbol = property_callable(&runtime, &mut first, &first_global, "Symbol");
    let second_symbol = property_callable(&runtime, &mut second, &second_global, "Symbol");
    let first_for = property_callable(&runtime, &mut first, first_symbol.as_object(), "for");
    let second_for = property_callable(&runtime, &mut second, second_symbol.as_object(), "for");
    let first_key_for = property_callable(&runtime, &mut first, first_symbol.as_object(), "keyFor");
    let first_prototype = first.symbol_prototype().unwrap();
    let second_prototype = second.symbol_prototype().unwrap();
    assert_ne!(first_prototype, second_prototype);

    for kind in WellKnownSymbol::ALL {
        let name = well_known_entries()
            .into_iter()
            .find_map(|(name, candidate)| (candidate == kind).then_some(name))
            .unwrap();
        let first_value = first
            .get_property(
                first_symbol.as_object(),
                &runtime.intern_property_key(name).unwrap(),
            )
            .unwrap();
        let second_value = second
            .get_property(
                second_symbol.as_object(),
                &runtime.intern_property_key(name).unwrap(),
            )
            .unwrap();
        assert_eq!(
            first_value, second_value,
            "well-known symbol split by realm"
        );
    }

    let registry_key = Value::String(JsString::try_from_utf8("cross-realm").unwrap());
    let first_registered = first
        .call(
            &first_for,
            Value::Undefined,
            std::slice::from_ref(&registry_key),
        )
        .unwrap();
    let second_registered = second
        .call(
            &second_for,
            Value::Undefined,
            std::slice::from_ref(&registry_key),
        )
        .unwrap();
    assert_eq!(first_registered, second_registered);
    assert_ne!(
        first
            .call(
                &first_symbol,
                Value::Undefined,
                &[Value::String(JsString::try_from_utf8("fresh").unwrap())],
            )
            .unwrap(),
        second
            .call(
                &second_symbol,
                Value::Undefined,
                &[Value::String(JsString::try_from_utf8("fresh").unwrap())],
            )
            .unwrap(),
    );

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
    let seven = runtime
        .new_symbol(Some(JsString::try_from_utf8("seven").unwrap()))
        .unwrap();
    let nine = runtime
        .new_symbol(Some(JsString::try_from_utf8("nine").unwrap()))
        .unwrap();
    let first_wrapper = expect_object(
        second
            .call(
                &first_object_value_of,
                Value::Symbol(seven.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        "first-realm Symbol wrapper",
    );
    let second_wrapper = expect_object(
        first
            .call(
                &second_object_value_of,
                Value::Symbol(nine.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        "second-realm Symbol wrapper",
    );
    assert_eq!(
        runtime.get_prototype_of(&first_wrapper).unwrap(),
        Some(first_prototype.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        runtime.get_prototype_of(&second_wrapper).unwrap(),
        Some(second_prototype.try_clone().expect("duplicate root"))
    );
    for (method, wrapper, expected) in [
        (&first_value_of, &first_wrapper, &seven),
        (&second_value_of, &first_wrapper, &seven),
        (&first_value_of, &second_wrapper, &nine),
        (&second_value_of, &second_wrapper, &nine),
    ] {
        assert_eq!(
            second
                .call(
                    method,
                    Value::Object(wrapper.try_clone().expect("duplicate root")),
                    &[]
                )
                .unwrap(),
            Value::Symbol(expected.try_clone().expect("duplicate root")),
            "Symbol wrapper branding must be realm-independent"
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
        "(function() { return Symbol('reader').__realmMarker; })",
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
        Value::String(JsString::try_from_utf8("FirstSymbol").unwrap()),
        false,
        false,
        true,
    );
    define_data_key(
        &runtime,
        &second_prototype,
        &tag,
        Value::String(JsString::try_from_utf8("SecondSymbol").unwrap()),
        false,
        false,
        true,
    );
    assert_eq!(
        second
            .call(
                &first_object_to_string,
                Value::Symbol(seven.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("[object FirstSymbol]").unwrap()),
        "Object.prototype.toString must box in the method's defining realm"
    );

    let first_type_error = intrinsic_prototype(&runtime, &mut first, "TypeError");
    let second_type_error = intrinsic_prototype(&runtime, &mut second, "TypeError");
    assert_ne!(first_type_error, second_type_error);
    let symbol_argument = runtime
        .new_symbol(Some(JsString::try_from_utf8("argument").unwrap()))
        .unwrap();
    assert_eq!(
        second.call(
            &first_symbol,
            Value::Undefined,
            &[Value::Symbol(symbol_argument)],
        ),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut second))
            .unwrap(),
        Some(first_type_error.try_clone().expect("duplicate root")),
        "Symbol conversion TypeError must use the constructor's defining realm"
    );
    assert_eq!(
        second.call(&first_key_for, Value::Undefined, &[Value::Int(1)]),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut second))
            .unwrap(),
        Some(first_type_error.try_clone().expect("duplicate root"))
    );
    let spoof = second.new_object().unwrap();
    assert_eq!(
        second.call(&first_value_of, Value::Object(spoof), &[]),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut second))
            .unwrap(),
        Some(first_type_error.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        second.construct(&first_symbol, &[]),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut second))
            .unwrap(),
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
            &first_symbol,
            Value::Undefined,
            &[Value::Object(throwing_input)],
        ),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut first))
            .unwrap(),
        Some(second_type_error),
        "an explicit user throw must retain the conversion function's realm"
    );

    assert_eq!(
        first
            .call(
                &first_key_for,
                Value::Undefined,
                std::slice::from_ref(&first_registered),
            )
            .unwrap(),
        registry_key
    );
    drop(second_global);
}

#[test]
fn symbol_primitives_do_not_retain_realms_but_wrappers_do_and_atoms_survive() {
    let runtime = host_runtime();
    let primitive = {
        let mut context = runtime.new_context().expect("create context");
        let global = context.global_object().unwrap();
        let constructor = property_callable(&runtime, &mut context, &global, "Symbol");
        call_symbol(
            &mut context,
            &constructor,
            &[Value::String(
                JsString::try_from_utf16([0x61, 0xd800, 0x62]).unwrap(),
            )],
        )
    };
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        0,
        "a realm-neutral Symbol primitive must not retain its creating context"
    );
    assert_eq!(
        runtime.symbol_description(&primitive).unwrap(),
        Some(JsString::try_from_utf16([0x61, 0xd800, 0x62]).unwrap()),
        "the symbol atom must outlive its creating context while rooted"
    );
    drop(primitive);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);

    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let object_prototype = context.object_prototype().unwrap();
        let object_value_of =
            property_callable(&runtime, &mut context, &object_prototype, "valueOf");
        let symbol = runtime
            .new_symbol(Some(JsString::try_from_utf8("wrapper-only").unwrap()))
            .unwrap();
        expect_object(
            context
                .call(&object_value_of, Value::Symbol(symbol), &[])
                .unwrap(),
            "Object.prototype.valueOf Symbol wrapper",
        )
    };
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1,
        "a live Symbol wrapper must retain its prototype and defining realm graph"
    );
    drop(wrapper);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").live,
        0,
        "Symbol contexts, prototypes, native functions, wrappers, and atoms must collect"
    );
}

fn well_known_entries() -> [(&'static str, WellKnownSymbol); 13] {
    [
        ("toPrimitive", WellKnownSymbol::ToPrimitive),
        ("iterator", WellKnownSymbol::Iterator),
        ("match", WellKnownSymbol::Match),
        ("matchAll", WellKnownSymbol::MatchAll),
        ("replace", WellKnownSymbol::Replace),
        ("search", WellKnownSymbol::Search),
        ("split", WellKnownSymbol::Split),
        ("toStringTag", WellKnownSymbol::ToStringTag),
        ("isConcatSpreadable", WellKnownSymbol::IsConcatSpreadable),
        ("hasInstance", WellKnownSymbol::HasInstance),
        ("species", WellKnownSymbol::Species),
        ("unscopables", WellKnownSymbol::Unscopables),
        ("asyncIterator", WellKnownSymbol::AsyncIterator),
    ]
}

fn define_data(
    runtime: &Runtime,
    object: &ObjectRef,
    name: &str,
    value: Value,
    writable: bool,
    enumerable: bool,
    configurable: bool,
) {
    define_data_key(
        runtime,
        object,
        &runtime.intern_property_key(name).unwrap(),
        value,
        writable,
        enumerable,
        configurable,
    );
}

fn call_symbol(context: &mut Context, callable: &CallableRef, arguments: &[Value]) -> SymbolRef {
    let Value::Symbol(value) = context.call(callable, Value::Undefined, arguments).unwrap() else {
        panic!("Symbol-producing call did not return a Symbol");
    };
    value
}
