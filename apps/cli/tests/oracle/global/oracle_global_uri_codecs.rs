use crate::object_graph_observation::{
    define_checked_builtin_data_key as define_data_key, eval_callable, global_callable,
    intrinsic_prototype,
};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_thrown_object as take_exception_object;
use quickjs_oxide::engine::api::{JsString, PropertyKey, RuntimeError, Value, WellKnownSymbol};

#[test]
fn global_uri_codec_errors_use_the_defining_realm() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_encode = global_callable(&runtime, &mut first, "encodeURI");
    let first_decode = global_callable(&runtime, &mut first, "decodeURIComponent");
    let first_function_prototype = first.function_prototype().unwrap();
    assert_eq!(
        runtime.get_prototype_of(first_encode.as_object()).unwrap(),
        Some(
            first_function_prototype
                .try_clone()
                .expect("duplicate root")
        )
    );
    assert_eq!(
        runtime.get_prototype_of(first_decode.as_object()).unwrap(),
        Some(first_function_prototype)
    );

    let first_type_error = intrinsic_prototype(&runtime, &mut first, "TypeError");
    let second_type_error = intrinsic_prototype(&runtime, &mut second, "TypeError");
    let first_uri_error = intrinsic_prototype(&runtime, &mut first, "URIError");
    let second_uri_error = intrinsic_prototype(&runtime, &mut second, "URIError");
    assert_ne!(first_type_error, second_type_error);
    assert_ne!(first_uri_error, second_uri_error);

    let foreign_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("foreign").unwrap()))
        .unwrap();
    assert_eq!(
        second.call(
            &first_encode,
            Value::Undefined,
            &[Value::Symbol(foreign_symbol)],
        ),
        Err(RuntimeError::Exception)
    );
    let symbol_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&symbol_error).unwrap(),
        Some(first_type_error)
    );

    assert_eq!(
        second.call(
            &first_decode,
            Value::Undefined,
            &[Value::String(JsString::try_from_utf8("%").unwrap())],
        ),
        Err(RuntimeError::Exception)
    );
    let malformed_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&malformed_error).unwrap(),
        Some(first_uri_error)
    );

    assert_eq!(
        second.construct(&first_encode, &[]),
        Err(RuntimeError::Exception)
    );
    let constructor_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&constructor_error).unwrap(),
        Some(second_type_error),
        "the caller realm rejects construction before entering the native"
    );

    let foreign_throw = eval_callable(
        &runtime,
        &mut second,
        "(function() { throw new URIError('foreign conversion'); })",
    );
    let foreign_object = second.new_object().unwrap();
    define_data_key(
        &runtime,
        &foreign_object,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(
            foreign_throw
                .as_object()
                .try_clone()
                .expect("duplicate root"),
        ),
    );
    assert_eq!(
        first.call(
            &first_encode,
            Value::Undefined,
            &[Value::Object(foreign_object)],
        ),
        Err(RuntimeError::Exception)
    );
    let user_error = take_exception_object(&mut first);
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(second_uri_error),
        "user-thrown conversion errors keep their originating realm"
    );
}

#[test]
fn global_uri_codec_keeps_its_defining_realm_alive_until_collection() {
    let runtime = host_runtime();
    let codec = {
        let mut context = runtime.new_context().expect("create context");
        global_callable(&runtime, &mut context, "encodeURIComponent")
    };

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(codec);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}
