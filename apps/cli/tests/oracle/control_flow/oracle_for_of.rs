use crate::runtime_observation::{host_runtime, host_runtime_context};

use crate::runtime_oracle::error_string_property;
use quickjs_oxide::engine::api::core::{JsString, RuntimeError, Value};

#[test]
fn top_level_for_await_uses_pinned_quickjs_syntax_diagnostic() {
    let (runtime, mut context) = host_runtime_context();
    let (source, expected) = ("for await(var value of 'a')value", "expecting '('");
    let Err(RuntimeError::Exception(Value::Object(error))) = context.compile(source) else {
        panic!("top-level for-await did not throw an Error object: {source}");
    };
    assert_eq!(
        error_string_property(&runtime, &mut context, &error, "message", source),
        expected,
        "{source}",
    );
}

#[test]
fn for_of_cross_realm_regression() {
    let runtime = host_runtime();
    let defining = runtime.new_context().expect("create context");
    let caller = runtime.new_context().expect("create context");
    let bytecode = defining
        .compile("(function(){var result='';for(var value of 'ab')result+=value;return result})()")
        .unwrap();
    assert_eq!(
        caller.execute(&bytecode).unwrap(),
        Value::String(JsString::try_from_utf8("ab").unwrap())
    );
}
