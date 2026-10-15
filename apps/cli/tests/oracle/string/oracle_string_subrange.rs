use crate::object_graph_observation::{define_builtin_data as define_data, intrinsic_prototype};
use crate::runtime_observation::assert_native_error as assert_native_type_error;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 `js_sub_string` (quickjs.c 3964-3991),
// `js_string_substring`/`js_string_substr`/`js_string_slice` (45983-46071),
// their adjacent function-list entries (46641-46643), and the shared
// saturated conversion helpers `JS_ToInt32SatFree`/`JS_ToInt32Clamp`
// (13124-13189).
//
// These vectors intentionally preserve QuickJS's implementation contract:
// substring clamps both indices and swaps them, substr applies a string-length
// offset to a negative start and treats its second argument as a length, while
// slice applies that offset independently to both indices and never swaps.
// All string observations containing unpaired surrogates are encoded as raw
// UTF-16 code units before crossing the process stdout boundary.

#[test]
fn string_subrange_defining_realms_and_user_throw_identity_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let substring = property_callable(&runtime, &mut defining, &defining_prototype, "substring");
    let substr = property_callable(&runtime, &mut defining, &defining_prototype, "substr");
    let slice = property_callable(&runtime, &mut defining, &defining_prototype, "slice");
    assert_eq!(
        runtime.get_prototype_of(substring.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_native_type_error(
        &runtime,
        &mut caller,
        &substring,
        Value::Null,
        &[Value::Int(0), Value::Int(1)],
        &defining_type_error,
    );
    let position_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("position").unwrap()))
        .unwrap();
    assert_native_type_error(
        &runtime,
        &mut caller,
        &slice,
        Value::String(JsString::try_from_utf8("abc").unwrap()),
        &[Value::Symbol(position_symbol)],
        &defining_type_error,
    );

    let sentinel = caller.new_object().unwrap();
    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "subrangeSentinel",
        Value::Object(sentinel.try_clone().expect("duplicate root")),
    );
    let throwing_start = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw subrangeSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(
            &substr,
            Value::String(JsString::try_from_utf8("abc").unwrap()),
            &[throwing_start, Value::Int(1)],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel.try_clone().expect("duplicate root"))),
        "start conversion did not preserve the user-thrown value",
    );

    let throwing_end = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw subrangeSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(
            &slice,
            Value::String(JsString::try_from_utf8("abc").unwrap()),
            &[Value::Int(0), throwing_end],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "end conversion did not preserve the user-thrown value",
    );

    assert_eq!(
        caller.construct(&substring, &[]),
        Err(RuntimeError::Exception),
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
fn string_subrange_callables_are_per_realm_distinct_and_collectable() {
    let runtime = host_runtime();
    let retained = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_prototype = first.string_prototype().unwrap();
        let second_prototype = second.string_prototype().unwrap();
        let first_substring =
            property_callable(&runtime, &mut first, &first_prototype, "substring");
        let first_substring_again =
            property_callable(&runtime, &mut first, &first_prototype, "substring");
        let first_substr = property_callable(&runtime, &mut first, &first_prototype, "substr");
        let first_slice = property_callable(&runtime, &mut first, &first_prototype, "slice");
        let second_substring =
            property_callable(&runtime, &mut second, &second_prototype, "substring");
        let second_substr = property_callable(&runtime, &mut second, &second_prototype, "substr");
        let second_slice = property_callable(&runtime, &mut second, &second_prototype, "slice");
        assert_eq!(first_substring, first_substring_again);
        assert_ne!(first_substring, first_substr);
        assert_ne!(first_substring, first_slice);
        assert_ne!(first_substr, first_slice);
        assert_ne!(first_substring, second_substring);
        assert_ne!(first_substr, second_substr);
        assert_ne!(first_slice, second_slice);
        assert_eq!(
            runtime
                .get_prototype_of(first_substring.as_object())
                .unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        (first_substring, first_substr, first_slice)
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
fn string_subrange_stack_overflow_uses_the_caller_realm_and_recovers() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let slice = property_callable(&runtime, &mut defining, &defining_prototype, "slice");
    let defining_internal_error = intrinsic_prototype(&runtime, &mut defining, "InternalError");
    let caller_internal_error = intrinsic_prototype(&runtime, &mut caller, "InternalError");
    assert_ne!(defining_internal_error, caller_internal_error);

    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "foreignSlice",
        Value::Object(slice.as_object().try_clone().expect("duplicate root")),
    );
    let Value::Object(error) = caller
        .eval(
            r#"(function(){
                var start=Object(),localCall=Function.prototype.call;
                function invoke(){
                    return localCall.call(foreignSlice,"abcdef",start,4)
                }
                start[Symbol.toPrimitive]=function(){
                    return invoke()
                };
                try{invoke()}catch(error){return error}
                return Object();
            })()"#,
        )
        .unwrap()
    else {
        panic!("recursive cross-realm call did not return an error object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_internal_error),
        "pre-dispatch stack overflow did not use the caller realm",
    );
    assert_eq!(
        caller
            .call(
                &slice,
                Value::String(JsString::try_from_utf8("recovered").unwrap()),
                &[Value::Int(2), Value::Int(7)],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("cover").unwrap()),
    );
}
