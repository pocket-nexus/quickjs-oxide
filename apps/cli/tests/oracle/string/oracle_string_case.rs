use crate::object_graph_observation::{define_builtin_data as define_data, intrinsic_prototype};
use crate::runtime_observation::assert_native_error;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_string_toLowerCase`, its Unicode 17 case
// conversion/property tables, and the four GenericMagic String prototype
// entries (quickjs.c 46215-46304 and 46656-46659; libunicode.c 51-190 and
// 347-376). The two locale-named variants deliberately share the ordinary
// lower/upper kernels and never inspect their locale arguments.
//
// Every raw String observation is encoded as hexadecimal UTF-16 code units
// before crossing the process boundary, preserving NUL and lone surrogates.

#[test]
fn string_case_defining_realms_user_throw_identity_and_caller_construct_error_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let lower = property_callable(&runtime, &mut defining, &defining_prototype, "toLowerCase");
    let locale_upper = property_callable(
        &runtime,
        &mut defining,
        &defining_prototype,
        "toLocaleUpperCase",
    );
    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_eq!(
        runtime.get_prototype_of(lower.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    assert_native_error(
        &runtime,
        &mut caller,
        &lower,
        Value::Null,
        &[],
        &defining_type_error,
    );
    let receiver_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("receiver").unwrap()))
        .unwrap();
    assert_native_error(
        &runtime,
        &mut caller,
        &locale_upper,
        Value::Symbol(receiver_symbol),
        &[],
        &defining_type_error,
    );

    let sentinel = caller.new_object().unwrap();
    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "caseSentinel",
        Value::Object(sentinel.try_clone().expect("duplicate root")),
    );
    let throwing_receiver = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw caseSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(&lower, throwing_receiver, &[]),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "receiver conversion did not preserve the user-thrown value",
    );

    assert_eq!(caller.construct(&lower, &[]), Err(RuntimeError::Exception));
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(caller_type_error),
        "non-constructor rejection did not use the caller realm",
    );
}

#[test]
fn string_case_callables_are_per_realm_distinct_and_collectable() {
    let runtime = host_runtime();
    let retained = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_prototype = first.string_prototype().unwrap();
        let second_prototype = second.string_prototype().unwrap();
        let names = [
            "toLowerCase",
            "toUpperCase",
            "toLocaleLowerCase",
            "toLocaleUpperCase",
        ];
        let first_functions =
            names.map(|name| property_callable(&runtime, &mut first, &first_prototype, name));
        let second_functions =
            names.map(|name| property_callable(&runtime, &mut second, &second_prototype, name));
        for index in 0..names.len() {
            assert_eq!(
                first_functions[index],
                property_callable(&runtime, &mut first, &first_prototype, names[index]),
            );
            assert_ne!(first_functions[index], second_functions[index]);
            for other in (index + 1)..names.len() {
                assert_ne!(first_functions[index], first_functions[other]);
            }
        }
        assert_eq!(
            runtime
                .get_prototype_of(first_functions[0].as_object())
                .unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        first_functions
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

#[test]
fn string_case_stack_overflow_uses_the_caller_realm_and_recovers() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let lower = property_callable(&runtime, &mut defining, &defining_prototype, "toLowerCase");
    let defining_internal_error = intrinsic_prototype(&runtime, &mut defining, "InternalError");
    let caller_internal_error = intrinsic_prototype(&runtime, &mut caller, "InternalError");
    assert_ne!(defining_internal_error, caller_internal_error);

    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "foreignLower",
        Value::Object(lower.as_object().try_clone().expect("duplicate root")),
    );
    let Value::Object(error) = caller
        .eval(
            r#"(function(){
                var receiver=Object(),localCall=Function.prototype.call;
                function invoke(){return localCall.call(foreignLower,receiver)}
                receiver[Symbol.toPrimitive]=function(){return invoke()};
                try{invoke()}catch(error){return error}
                return Object();
            })()"#,
        )
        .unwrap()
    else {
        panic!("recursive cross-realm String case conversion did not return an error object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_internal_error),
        "pre-dispatch stack overflow did not use the caller realm",
    );
    assert_eq!(
        caller
            .call(
                &lower,
                Value::String(JsString::try_from_utf8("AbC").unwrap()),
                &[],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("abc").unwrap()),
    );
}
