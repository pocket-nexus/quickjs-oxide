//! Evaluation helpers shared by the engine's unit tests.

use crate::engine::api::{Context, Runtime, Value};
use crate::engine::object::ObjectRef;

/// A fresh runtime and one context in it.
pub(crate) fn test_context() -> (Runtime, Context) {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    (runtime, context)
}

/// Evaluate a script that must complete with a String.
pub(crate) fn eval_string(context: &mut Context, source: &str) -> String {
    let Value::String(value) = context.eval(source).unwrap() else {
        panic!("script did not return a String: {source}");
    };
    value.to_utf8_lossy()
}

/// Evaluate a self-checking script that returns `true` on success.
#[track_caller]
pub(crate) fn assert_script_true(context: &mut Context, source: &str) {
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true), "{source}");
}

/// Evaluate a self-checking script that returns `"ok"` on success.
pub(crate) fn assert_script(context: &mut Context, source: &str) {
    assert_eq!(eval_string(context, source), "ok");
}

pub(crate) fn eval_object(context: &mut Context, source: &str, description: &str) -> ObjectRef {
    let Value::Object(object) = context
        .eval(source)
        .unwrap_or_else(|error| panic!("{description} was rejected: {error}"))
    else {
        panic!("{description} was not an Object");
    };
    object
}

/// Take the pending exception, which must be an Object.
pub(crate) fn take_exception_object(context: &mut Context, description: &str) -> ObjectRef {
    let Value::Object(object) = context
        .take_exception()
        .unwrap_or_else(|failure| panic!("take {description}: {failure}"))
        .unwrap_or_else(|| panic!("{description} did not leave an exception"))
    else {
        panic!("{description} exception was not an Object");
    };
    object
}
