use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::{
    define_named_builtin_data as define_data, define_to_primitive,
    eval_source_callable as eval_callable, intrinsic_prototype,
};
use crate::runtime_observation::{property_callable, take_error_object as take_exception_object};
use quickjs_oxide::engine::api::core::{
    Context, JsString, ObjectRef, Runtime, RuntimeError, Value,
};

// Pins the adjacent `indexOf`/`lastIndexOf` String.prototype slice to QuickJS
// 2026-06-04. The complete constructor table is now published, while these
// probes retain their direct prototype access to isolate the shared search
// kernel from the later regexp-aware `includes` entry.

#[test]
fn string_index_search_cross_realm_errors_and_user_throws_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_string = defining.string_prototype().unwrap();
    let index_of = property_callable(&runtime, &mut defining, &defining_string, "indexOf");
    let last_index_of = property_callable(&runtime, &mut defining, &defining_string, "lastIndexOf");

    assert_eq!(
        caller
            .call(
                &index_of,
                Value::String(js_string("abcabc")),
                &[Value::String(js_string("cab"))],
            )
            .unwrap(),
        Value::Int(2),
    );

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    assert_eq!(
        caller.call(&index_of, Value::Null, &[Value::String(js_string("x"))],),
        Err(RuntimeError::Exception),
    );
    let framework_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&framework_error).unwrap(),
        Some(defining_type_error),
        "null-receiver error did not use the method's defining realm",
    );

    let caller_range_error = intrinsic_prototype(&runtime, &mut caller, "RangeError");
    let throwing_search = caller.new_object().unwrap();
    define_method(
        &runtime,
        &mut caller,
        &throwing_search,
        "toString",
        "(function(){throw new RangeError('search')})",
    );
    assert_eq!(
        caller.call(
            &index_of,
            Value::String(js_string("abc")),
            &[Value::Object(throwing_search)],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller);
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_range_error),
        "user search conversion throw lost its callable realm",
    );

    let sentinel = caller.new_object().unwrap();
    let caller_global = caller.global_object().unwrap();
    define_data(
        &runtime,
        &caller_global,
        "stringSearchSentinel",
        Value::Object(sentinel.try_clone().expect("duplicate root")),
    );
    let throwing_position = caller.new_object().unwrap();
    let position_conversion = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw stringSearchSentinel})",
    );
    define_to_primitive(&runtime, &throwing_position, position_conversion);
    assert_eq!(
        caller.call(
            &last_index_of,
            Value::String(js_string("abc")),
            &[
                Value::String(js_string("a")),
                Value::Object(throwing_position),
            ],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "user position conversion throw identity changed",
    );
}

fn define_method(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
    source: &str,
) {
    let callable = eval_callable(runtime, context, source);
    define_data(
        runtime,
        object,
        name,
        Value::Object(callable.as_object().try_clone().expect("duplicate root")),
    );
}

fn js_string(value: &str) -> JsString {
    JsString::try_from_utf8(value).unwrap()
}
