use crate::object_graph_observation::{
    define_builtin_data_key as define_data_key, eval_source_callable as eval_callable,
    intrinsic_prototype,
};
use crate::runtime_observation::assert_native_error as assert_native_type_error;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{
    AccessorValue, CallableRef, DescriptorField, JsString, ObjectRef, OrdinaryPropertyDescriptor,
    PropertyKey, Runtime, RuntimeError, Value, WellKnownSymbol,
};

// Pins QuickJS 2026-06-04 `js_string_includes` (quickjs.c 45516-45578),
// `js_is_regexp` (47487-47499), and the adjacent function-list entries
// (46634-46636). The three methods deliberately share one magic-selected
// kernel but retain distinct position defaults and search ranges.
//
// Proxy, TypedArray, and genuine RegExp values participate in the same
// differential. Module namespace conversion remains a separate
// module/object-model boundary.

#[test]
fn string_includes_defining_realms_and_user_throw_identity_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let includes = property_callable(&runtime, &mut defining, &defining_prototype, "includes");
    let starts_with = property_callable(&runtime, &mut defining, &defining_prototype, "startsWith");
    let ends_with = property_callable(&runtime, &mut defining, &defining_prototype, "endsWith");
    assert_eq!(
        runtime.get_prototype_of(includes.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_native_type_error(
        &runtime,
        &mut caller,
        &includes,
        Value::Null,
        &[Value::String(JsString::try_from_utf8("x").unwrap())],
        &defining_type_error,
    );
    let search_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("search").unwrap()))
        .unwrap();
    assert_native_type_error(
        &runtime,
        &mut caller,
        &starts_with,
        Value::String(JsString::try_from_utf8("abc").unwrap()),
        &[Value::Symbol(search_symbol)],
        &defining_type_error,
    );
    let regexp_like = caller.new_object().unwrap();
    define_data_key(
        &runtime,
        &regexp_like,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::Match)
                .expect("well-known symbol"),
        ),
        Value::Bool(true),
    );
    assert_native_type_error(
        &runtime,
        &mut caller,
        &ends_with,
        Value::String(JsString::try_from_utf8("abc").unwrap()),
        &[Value::Object(regexp_like)],
        &defining_type_error,
    );
    let position_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("position").unwrap()))
        .unwrap();
    assert_native_type_error(
        &runtime,
        &mut caller,
        &includes,
        Value::String(JsString::try_from_utf8("abc").unwrap()),
        &[
            Value::String(JsString::try_from_utf8("a").unwrap()),
            Value::Symbol(position_symbol),
        ],
        &defining_type_error,
    );

    let sentinel = caller.new_object().unwrap();
    let sentinel_key = runtime.intern_property_key("includesSentinel").unwrap();
    assert!(
        caller
            .set_property(
                &caller.global_object().unwrap(),
                &sentinel_key,
                Value::Object(sentinel.try_clone().expect("duplicate root")),
            )
            .unwrap()
    );
    let throwing_getter = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw includesSentinel})",
    );
    let search = caller.new_object().unwrap();
    define_accessor_key(
        &runtime,
        &search,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::Match)
                .expect("well-known symbol"),
        ),
        Some(throwing_getter),
    );
    assert_eq!(
        caller.call(
            &includes,
            Value::String(JsString::try_from_utf8("abc").unwrap()),
            &[Value::Object(search)],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "Symbol.match getter throw identity was not preserved",
    );

    assert_eq!(
        caller.construct(&includes, &[]),
        Err(RuntimeError::Exception)
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(caller_type_error),
        "non-constructor rejection did not use the caller realm",
    );
}

#[test]
fn string_includes_callables_are_per_realm_and_collectable() {
    let runtime = host_runtime();
    let retained = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_prototype = first.string_prototype().unwrap();
        let second_prototype = second.string_prototype().unwrap();
        let first_method = property_callable(&runtime, &mut first, &first_prototype, "includes");
        let first_again = property_callable(&runtime, &mut first, &first_prototype, "includes");
        let second_method = property_callable(&runtime, &mut second, &second_prototype, "includes");
        assert_eq!(first_method, first_again);
        assert_ne!(first_method, second_method);
        assert_eq!(
            runtime.get_prototype_of(first_method.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        first_method
    };
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(retained);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}

fn define_accessor_key(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
    getter: Option<CallableRef>,
) {
    assert!(
        runtime
            .define_own_property(
                object,
                key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(match getter {
                        Some(getter) => AccessorValue::Callable(getter),
                        None => AccessorValue::Undefined,
                    }),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}
