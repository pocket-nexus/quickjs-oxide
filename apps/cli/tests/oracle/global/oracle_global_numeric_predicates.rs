use crate::object_graph_observation::{
    define_checked_builtin_data as define_data, define_checked_builtin_data_key as define_data_key,
    eval_callable, global_callable, intrinsic_prototype,
};
use crate::runtime_observation::error_text;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_thrown_object as take_exception_object;
use quickjs_oxide::engine::api::{JsString, PropertyKey, RuntimeError, Value, WellKnownSymbol};

#[test]
fn global_numeric_predicate_errors_use_the_defining_realm() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_nan = global_callable(&runtime, &mut first, "isNaN");
    let first_finite = global_callable(&runtime, &mut first, "isFinite");
    let first_function_prototype = first.function_prototype().unwrap();
    assert_eq!(
        runtime.get_prototype_of(first_nan.as_object()).unwrap(),
        Some(
            first_function_prototype
                .try_clone()
                .expect("duplicate root")
        )
    );
    assert_eq!(
        runtime.get_prototype_of(first_finite.as_object()).unwrap(),
        Some(first_function_prototype)
    );

    let first_type_error = intrinsic_prototype(&runtime, &mut first, "TypeError");
    let second_type_error = intrinsic_prototype(&runtime, &mut second, "TypeError");
    assert_ne!(first_type_error, second_type_error);

    assert_eq!(
        second.construct(&first_nan, &[]),
        Err(RuntimeError::Exception)
    );
    let constructor_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&constructor_error).unwrap(),
        Some(second_type_error.try_clone().expect("duplicate root")),
        "non-constructor rejection must use the caller realm"
    );

    let symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("foreign").unwrap()))
        .unwrap();
    assert_eq!(
        second.call(&first_nan, Value::Undefined, &[Value::Symbol(symbol)]),
        Err(RuntimeError::Exception)
    );
    let symbol_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&symbol_error).unwrap(),
        Some(first_type_error.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        error_text(&runtime, &mut second, &symbol_error, "message"),
        "cannot convert symbol to number"
    );

    let second_global = second.global_object().unwrap();
    let bad_result = second.new_object().unwrap();
    define_data(
        &runtime,
        &second_global,
        "badPredicatePrimitive",
        Value::Object(bad_result),
    );
    let bad_conversion = eval_callable(
        &runtime,
        &mut second,
        "(function() { return badPredicatePrimitive; })",
    );
    let bad_input = second.new_object().unwrap();
    define_data_key(
        &runtime,
        &bad_input,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(
            bad_conversion
                .as_object()
                .try_clone()
                .expect("duplicate root"),
        ),
    );
    assert_eq!(
        second.call(&first_finite, Value::Undefined, &[Value::Object(bad_input)],),
        Err(RuntimeError::Exception)
    );
    let framework_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&framework_error).unwrap(),
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
    );
    assert_eq!(
        first.call(
            &first_nan,
            Value::Undefined,
            &[Value::Object(throwing_input)],
        ),
        Err(RuntimeError::Exception)
    );
    let user_error = take_exception_object(&mut first);
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(second_type_error)
    );
}

#[test]
fn global_numeric_predicate_keeps_its_defining_realm_alive_until_collection() {
    let runtime = host_runtime();
    let predicate = {
        let mut context = runtime.new_context().expect("create context");
        global_callable(&runtime, &mut context, "isNaN")
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(predicate);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}
