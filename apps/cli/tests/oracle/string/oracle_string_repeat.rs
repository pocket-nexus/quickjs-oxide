use crate::object_graph_observation::{define_builtin_data as define_data, intrinsic_prototype};
use crate::runtime_observation::assert_native_error;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_string_repeat` (quickjs.c 46144-46187),
// its String prototype table entry (46644), `JS_ToInt64SatFree` (13191-13234),
// `JS_ToStringCheckObject` (13670-13676), and the 30-bit String length cap
// (212). QuickJS converts the receiver first, converts and validates the count
// second, returns the converted source for an empty String or count one, and
// otherwise builds one flat String using exact UTF-16 code-unit repetition.
//
// Raw UTF-16 observations encode code units as hexadecimal before crossing the
// process boundary, so lone surrogates never depend on terminal UTF-8 handling.

#[test]
fn string_repeat_defining_realms_and_user_throw_identity_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let repeat = property_callable(&runtime, &mut defining, &defining_prototype, "repeat");
    assert_eq!(
        runtime.get_prototype_of(repeat.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let defining_range_error = intrinsic_prototype(&runtime, &mut defining, "RangeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_native_error(
        &runtime,
        &mut caller,
        &repeat,
        Value::Null,
        &[Value::Int(1)],
        &defining_type_error,
    );
    let count_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("count").unwrap()))
        .unwrap();
    assert_native_error(
        &runtime,
        &mut caller,
        &repeat,
        Value::String(JsString::try_from_utf8("abc").unwrap()),
        &[Value::Symbol(count_symbol)],
        &defining_type_error,
    );
    assert_native_error(
        &runtime,
        &mut caller,
        &repeat,
        Value::String(JsString::try_from_utf8("abc").unwrap()),
        &[Value::Int(-1)],
        &defining_range_error,
    );

    let sentinel = caller.new_object().unwrap();
    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "repeatSentinel",
        Value::Object(sentinel.try_clone().expect("duplicate root")),
    );
    let throwing_count = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw repeatSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(
            &repeat,
            Value::String(JsString::try_from_utf8("abc").unwrap()),
            &[throwing_count],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "count conversion did not preserve the user-thrown value",
    );

    assert_eq!(caller.construct(&repeat, &[]), Err(RuntimeError::Exception));
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(caller_type_error),
        "non-constructor rejection did not use the caller realm",
    );
}

#[test]
fn string_repeat_callables_are_per_realm_distinct_and_collectable() {
    let runtime = host_runtime();
    let retained = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_prototype = first.string_prototype().unwrap();
        let second_prototype = second.string_prototype().unwrap();
        let first_repeat = property_callable(&runtime, &mut first, &first_prototype, "repeat");
        let first_repeat_again =
            property_callable(&runtime, &mut first, &first_prototype, "repeat");
        let first_slice = property_callable(&runtime, &mut first, &first_prototype, "slice");
        let second_repeat = property_callable(&runtime, &mut second, &second_prototype, "repeat");
        assert_eq!(first_repeat, first_repeat_again);
        assert_ne!(first_repeat, first_slice);
        assert_ne!(first_repeat, second_repeat);
        assert_eq!(
            runtime.get_prototype_of(first_repeat.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        first_repeat
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
fn string_repeat_stack_overflow_uses_the_caller_realm_and_recovers() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let repeat = property_callable(&runtime, &mut defining, &defining_prototype, "repeat");
    let defining_internal_error = intrinsic_prototype(&runtime, &mut defining, "InternalError");
    let caller_internal_error = intrinsic_prototype(&runtime, &mut caller, "InternalError");
    assert_ne!(defining_internal_error, caller_internal_error);

    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "foreignRepeat",
        Value::Object(repeat.as_object().try_clone().expect("duplicate root")),
    );
    let Value::Object(error) = caller
        .eval(
            r#"(function(){
                var count=Object(),localCall=Function.prototype.call;
                function invoke(){return localCall.call(foreignRepeat,"x",count)}
                count[Symbol.toPrimitive]=function(){return invoke()};
                try{invoke()}catch(error){return error}
                return Object();
            })()"#,
        )
        .unwrap()
    else {
        panic!("recursive cross-realm repeat did not return an error object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_internal_error),
        "pre-dispatch stack overflow did not use the caller realm",
    );
    assert_eq!(
        caller
            .call(
                &repeat,
                Value::String(JsString::try_from_utf8("recovered").unwrap()),
                &[Value::Int(2)],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("recoveredrecovered").unwrap()),
    );
}
