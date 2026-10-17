use crate::object_graph_observation::{define_to_primitive, eval_callable, intrinsic_prototype};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{property_callable, take_error_object as take_exception_object};
use quickjs_oxide::engine::api::core::{JsBigInt, JsString, RuntimeError, Value};

#[test]
fn string_utf16_prefix_cross_realm_and_error_realms_match_quickjs() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_prototype = first.string_prototype().unwrap();
    let at = property_callable(&runtime, &mut first, &first_prototype, "at");
    assert_eq!(
        second
            .call(
                &at,
                Value::String(JsString::try_from_utf16([0xd83d, 0xde00]).unwrap()),
                &[Value::Int(0)],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf16([0xd83d]).unwrap())
    );

    let first_type_error = intrinsic_prototype(&runtime, &mut first, "TypeError");
    assert_eq!(
        second.call(
            &at,
            Value::String(JsString::try_from_utf8("x").unwrap()),
            &[Value::BigInt(JsBigInt::one())],
        ),
        Err(RuntimeError::Exception)
    );
    let framework_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&framework_error).unwrap(),
        Some(first_type_error)
    );

    let second_range_error = intrinsic_prototype(&runtime, &mut second, "RangeError");
    let throwing_index = second.new_object().unwrap();
    let user_throw = eval_callable(
        &runtime,
        &mut second,
        "(function(){throw new RangeError('user');})",
    );
    define_to_primitive(&runtime, &throwing_index, user_throw);
    assert_eq!(
        first.call(
            &at,
            Value::String(JsString::try_from_utf8("x").unwrap()),
            &[Value::Object(throwing_index)],
        ),
        Err(RuntimeError::Exception)
    );
    let user_error = take_exception_object(&mut first);
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(second_range_error)
    );
}

#[test]
fn string_utf16_prefix_method_retains_then_releases_its_realm_graph() {
    let runtime = host_runtime();
    let method = {
        let mut context = runtime.new_context().expect("create context");
        let prototype = context.string_prototype().unwrap();
        property_callable(&runtime, &mut context, &prototype, "at")
    };
    runtime.run_gc().unwrap();
    drop(method);
    runtime.run_gc().unwrap();
}
