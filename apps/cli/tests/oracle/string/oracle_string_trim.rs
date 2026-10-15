use crate::object_graph_observation::{define_builtin_data as define_data, intrinsic_prototype};
use crate::runtime_observation::assert_native_error;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_string_trim` (quickjs.c 46189-46213), its
// GenericMagic table entries and alias entries (46649-46653),
// `JS_ToStringCheckObject` (13670-13676), `js_sub_string` (3964-3990), and
// `lre_is_space`/`char_range_s` (libunicode.h 140-166; libunicode.c
// 1885-1917). The magic bitmask uses one for the leading end and two for the
// trailing end. `trimRight` copies the `trimEnd` function value and `trimLeft`
// copies the `trimStart` function value, so aliases share canonical callable
// identity and names initially while their writable properties mutate
// independently afterward.
//
// Raw UTF-16 observations encode code units as hexadecimal before crossing the
// process boundary, so lone surrogates never depend on terminal UTF-8 handling.

#[test]
fn string_trim_defining_realms_user_throw_identity_and_caller_construct_error_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let trim = property_callable(&runtime, &mut defining, &defining_prototype, "trim");
    let trim_end = property_callable(&runtime, &mut defining, &defining_prototype, "trimEnd");
    let trim_right = property_callable(&runtime, &mut defining, &defining_prototype, "trimRight");
    let trim_start = property_callable(&runtime, &mut defining, &defining_prototype, "trimStart");
    assert_eq!(trim_end, trim_right);
    assert_eq!(
        runtime.get_prototype_of(trim.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_native_error(
        &runtime,
        &mut caller,
        &trim,
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
        &trim_start,
        Value::Symbol(receiver_symbol),
        &[],
        &defining_type_error,
    );

    let sentinel = caller.new_object().unwrap();
    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "trimSentinel",
        Value::Object(sentinel.try_clone().expect("duplicate root")),
    );
    let throwing_receiver = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw trimSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(&trim_end, throwing_receiver, &[]),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "receiver conversion did not preserve the user-thrown value",
    );

    assert_eq!(
        caller.construct(&trim_right, &[]),
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
fn string_trim_callables_aliases_are_per_realm_distinct_and_collectable() {
    let runtime = host_runtime();
    let retained = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_prototype = first.string_prototype().unwrap();
        let second_prototype = second.string_prototype().unwrap();
        let first_trim = property_callable(&runtime, &mut first, &first_prototype, "trim");
        let first_trim_again = property_callable(&runtime, &mut first, &first_prototype, "trim");
        let first_end = property_callable(&runtime, &mut first, &first_prototype, "trimEnd");
        let first_right = property_callable(&runtime, &mut first, &first_prototype, "trimRight");
        let first_start = property_callable(&runtime, &mut first, &first_prototype, "trimStart");
        let first_left = property_callable(&runtime, &mut first, &first_prototype, "trimLeft");
        let second_trim = property_callable(&runtime, &mut second, &second_prototype, "trim");
        let second_end = property_callable(&runtime, &mut second, &second_prototype, "trimEnd");
        let second_right = property_callable(&runtime, &mut second, &second_prototype, "trimRight");
        let second_start = property_callable(&runtime, &mut second, &second_prototype, "trimStart");
        let second_left = property_callable(&runtime, &mut second, &second_prototype, "trimLeft");
        assert_eq!(first_trim, first_trim_again);
        assert_eq!(first_end, first_right);
        assert_eq!(first_start, first_left);
        assert_eq!(second_end, second_right);
        assert_eq!(second_start, second_left);
        assert_ne!(first_trim, first_end);
        assert_ne!(first_trim, first_start);
        assert_ne!(first_end, first_start);
        assert_ne!(first_trim, second_trim);
        assert_ne!(first_end, second_end);
        assert_ne!(first_start, second_start);
        assert_eq!(
            runtime.get_prototype_of(first_end.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        (first_trim, first_end, first_start)
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
fn string_trim_stack_overflow_uses_the_caller_realm_and_recovers() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let trim = property_callable(&runtime, &mut defining, &defining_prototype, "trim");
    let defining_internal_error = intrinsic_prototype(&runtime, &mut defining, "InternalError");
    let caller_internal_error = intrinsic_prototype(&runtime, &mut caller, "InternalError");
    assert_ne!(defining_internal_error, caller_internal_error);

    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "foreignTrim",
        Value::Object(trim.as_object().try_clone().expect("duplicate root")),
    );
    let Value::Object(error) = caller
        .eval(
            r#"(function(){
                var receiver=Object(),localCall=Function.prototype.call;
                function invoke(){return localCall.call(foreignTrim,receiver)}
                receiver[Symbol.toPrimitive]=function(){return invoke()};
                try{invoke()}catch(error){return error}
                return Object();
            })()"#,
        )
        .unwrap()
    else {
        panic!("recursive cross-realm trim did not return an error object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_internal_error),
        "pre-dispatch stack overflow did not use the caller realm",
    );
    assert_eq!(
        caller
            .call(
                &trim,
                Value::String(JsString::try_from_utf8("  x  ").unwrap()),
                &[],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("x").unwrap()),
    );
}
