// Host-API observation helpers shared by the primitive-intrinsic oracles
// (Number, Boolean, BigInt, Symbol). Each topic passes its own value renderer
// so its frozen spelling stays local.
use crate::runtime_observation::{error_text, take_error_object};
use quickjs_oxide::engine::api::{CallableRef, Context, Runtime, RuntimeError, Value};

/// Take the pending Error object and render it as `throw:name:message`.
pub(crate) fn take_error(runtime: &Runtime, context: &mut Context) -> String {
    let error = take_error_object(context);
    let name = error_text(runtime, context, &error, "name");
    let message = error_text(runtime, context, &error, "message");
    format!("throw:{name}:{message}")
}

/// Evaluate a function source, reporting a JavaScript throw as its Error text.
pub(crate) fn eval_callable(runtime: &Runtime, context: &mut Context, source: &str) -> CallableRef {
    let value = match context.eval(source) {
        Ok(value) => value,
        Err(RuntimeError::Exception) => {
            panic!(
                "callable source threw: {source:?}: {}",
                take_error(runtime, context)
            )
        }
        Err(error) => panic!("callable source failed: {source:?}: {error}"),
    };
    let Value::Object(object) = value else {
        panic!("callable source did not produce an object: {source:?}");
    };
    runtime
        .as_callable(&object)
        .unwrap()
        .unwrap_or_else(|| panic!("source did not produce a callable: {source:?}"))
}
