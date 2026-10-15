use crate::object_graph_observation::{define_builtin_data as define_data, intrinsic_prototype};
use crate::runtime_observation::assert_native_error;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_string_pad` (quickjs.c 46073-46142), the
// `padEnd`/`padStart` GenericMagic entries and selectors (46647-46648),
// `JS_ToInt32SatFree`/`JS_ToInt32Sat` (13124-13172), and the 30-bit String
// length cap (212). QuickJS uses magic 1 for padEnd and 0 for padStart.
//
// The exact ordering matters: convert the receiver, saturate the target length,
// return immediately when the source is already long enough, then convert the
// optional filler. An empty converted filler also returns before the length-cap
// check. Padding repeats and truncates raw UTF-16 code units, so it may retain
// lone surrogates or split an astral pair.

#[test]
fn string_pad_defining_realms_and_user_throw_identity_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let pad_end = property_callable(&runtime, &mut defining, &defining_prototype, "padEnd");
    let pad_start = property_callable(&runtime, &mut defining, &defining_prototype, "padStart");
    assert_eq!(
        runtime.get_prototype_of(pad_end.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let defining_range_error = intrinsic_prototype(&runtime, &mut defining, "RangeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_native_error(
        &runtime,
        &mut caller,
        &pad_end,
        Value::Null,
        &[
            Value::Int(4),
            Value::String(JsString::try_from_utf8("_").unwrap()),
        ],
        &defining_type_error,
    );
    let target_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("target").unwrap()))
        .unwrap();
    assert_native_error(
        &runtime,
        &mut caller,
        &pad_start,
        Value::String(JsString::try_from_utf8("x").unwrap()),
        &[Value::Symbol(target_symbol)],
        &defining_type_error,
    );
    let filler_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("filler").unwrap()))
        .unwrap();
    assert_native_error(
        &runtime,
        &mut caller,
        &pad_end,
        Value::String(JsString::try_from_utf8("x").unwrap()),
        &[Value::Int(4), Value::Symbol(filler_symbol)],
        &defining_type_error,
    );
    assert_native_error(
        &runtime,
        &mut caller,
        &pad_start,
        Value::String(JsString::try_from_utf8("x").unwrap()),
        &[
            Value::Int(i32::MAX),
            Value::String(JsString::try_from_utf8("_").unwrap()),
        ],
        &defining_range_error,
    );

    let sentinel = caller.new_object().unwrap();
    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "padSentinel",
        Value::Object(sentinel.try_clone().expect("duplicate root")),
    );
    let throwing_target = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw padSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(
            &pad_end,
            Value::String(JsString::try_from_utf8("x").unwrap()),
            &[
                throwing_target,
                Value::String(JsString::try_from_utf8("_").unwrap()),
            ],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel.try_clone().expect("duplicate root"))),
        "target conversion did not preserve the user-thrown value",
    );

    let throwing_filler = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw padSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(
            &pad_start,
            Value::String(JsString::try_from_utf8("x").unwrap()),
            &[Value::Int(4), throwing_filler],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "filler conversion did not preserve the user-thrown value",
    );

    assert_eq!(
        caller.construct(&pad_end, &[]),
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
fn string_pad_callables_are_per_realm_distinct_and_collectable() {
    let runtime = host_runtime();
    let retained = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_prototype = first.string_prototype().unwrap();
        let second_prototype = second.string_prototype().unwrap();
        let first_end = property_callable(&runtime, &mut first, &first_prototype, "padEnd");
        let first_end_again = property_callable(&runtime, &mut first, &first_prototype, "padEnd");
        let first_start = property_callable(&runtime, &mut first, &first_prototype, "padStart");
        let first_repeat = property_callable(&runtime, &mut first, &first_prototype, "repeat");
        let second_end = property_callable(&runtime, &mut second, &second_prototype, "padEnd");
        let second_start = property_callable(&runtime, &mut second, &second_prototype, "padStart");
        assert_eq!(first_end, first_end_again);
        assert_ne!(first_end, first_start);
        assert_ne!(first_end, first_repeat);
        assert_ne!(first_start, first_repeat);
        assert_ne!(first_end, second_end);
        assert_ne!(first_start, second_start);
        assert_eq!(
            runtime.get_prototype_of(first_end.as_object()).unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        (first_end, first_start)
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
fn string_pad_stack_overflow_uses_the_caller_realm_and_recovers() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let pad_end = property_callable(&runtime, &mut defining, &defining_prototype, "padEnd");
    let defining_internal_error = intrinsic_prototype(&runtime, &mut defining, "InternalError");
    let caller_internal_error = intrinsic_prototype(&runtime, &mut caller, "InternalError");
    assert_ne!(defining_internal_error, caller_internal_error);

    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "foreignPadEnd",
        Value::Object(pad_end.as_object().try_clone().expect("duplicate root")),
    );
    let Value::Object(error) = caller
        .eval(
            r#"(function(){
                var target=Object(),localCall=Function.prototype.call;
                function invoke(){return localCall.call(foreignPadEnd,"x",target,"_")}
                target[Symbol.toPrimitive]=function(){return invoke()};
                try{invoke()}catch(error){return error}
                return Object();
            })()"#,
        )
        .unwrap()
    else {
        panic!("recursive cross-realm padEnd did not return an error object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_internal_error),
        "pre-dispatch stack overflow did not use the caller realm",
    );
    assert_eq!(
        caller
            .call(
                &pad_end,
                Value::String(JsString::try_from_utf8("x").unwrap()),
                &[
                    Value::Int(4),
                    Value::String(JsString::try_from_utf8("_").unwrap()),
                ],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("x___").unwrap()),
    );
}
