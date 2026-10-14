// Host-API observation helpers shared by the primitive-intrinsic oracles
// (Number, Boolean, BigInt, Symbol). Each topic passes its own value renderer
// so its frozen spelling stays local.
use crate::object_graph_observation::{
    define_accessor, define_data_key_with_flags, define_global_key,
};
use crate::runtime_observation::{error_text, take_error_object};
use quickjs_oxide::engine::api::{
    CallableRef, Context, ObjectRef, PropertyKey, Runtime, RuntimeError, Value, WellKnownSymbol,
};

/// Take the pending Error object and render it as `throw:name:message`.
pub(crate) fn take_error(runtime: &Runtime, context: &mut Context) -> String {
    let error = take_error_object(context);
    let name = error_text(runtime, context, &error, "name");
    let message = error_text(runtime, context, &error, "message");
    format!("throw:{name}:{message}")
}

pub(crate) fn observe_eval(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    render: impl FnOnce(Value) -> String,
) -> String {
    match context.eval(source) {
        Ok(value) => render(value),
        Err(RuntimeError::Exception) => take_error(runtime, context),
        Err(error) => {
            panic!("Rust source failed outside JavaScript completion: {source:?}: {error}")
        }
    }
}

pub(crate) fn observe_call(
    runtime: &Runtime,
    context: &mut Context,
    callable: &CallableRef,
    this_value: Value,
    arguments: &[Value],
    render: impl FnOnce(Value) -> String,
) -> String {
    match context.call(callable, this_value, arguments) {
        Ok(value) => render(value),
        Err(RuntimeError::Exception) => take_error(runtime, context),
        Err(error) => panic!("Rust call failed outside JavaScript completion: {error}"),
    }
}

pub(crate) fn observe_construct(
    runtime: &Runtime,
    context: &mut Context,
    callable: &CallableRef,
    arguments: &[Value],
    render: impl FnOnce(Value) -> String,
) -> String {
    match context.construct(callable, arguments) {
        Ok(value) => render(value),
        Err(RuntimeError::Exception) => take_error(runtime, context),
        Err(error) => panic!("Rust construct failed outside JavaScript completion: {error}"),
    }
}

pub(crate) fn observe_construct_with_new_target(
    runtime: &Runtime,
    context: &mut Context,
    callable: &CallableRef,
    new_target: &CallableRef,
    arguments: &[Value],
    render: impl FnOnce(Value) -> String,
) -> String {
    match context.construct_with_new_target(callable, new_target, arguments) {
        Ok(value) => render(value),
        Err(RuntimeError::Exception) => take_error(runtime, context),
        Err(error) => panic!("Rust construct failed outside JavaScript completion: {error}"),
    }
}

/// Render rooted values with `render` and join them with `|`.
pub(crate) fn join_values(values: &[Value], render: impl FnMut(Value) -> String) -> String {
    values
        .iter()
        .map(|value| value.try_clone().expect("duplicate root"))
        .map(render)
        .collect::<Vec<_>>()
        .join("|")
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

/// An ordinary object whose `@@toPrimitive` is the function `source`.
pub(crate) fn conversion_object(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
) -> ObjectRef {
    let object = context.new_object().unwrap();
    let conversion = eval_callable(runtime, context, source);
    define_data_key_with_flags(
        runtime,
        &object,
        &PropertyKey::from(
            runtime
                .well_known_symbol(WellKnownSymbol::ToPrimitive)
                .expect("well-known symbol"),
        ),
        Value::Object(conversion.as_object().try_clone().expect("duplicate root")),
        true,
        false,
        true,
    );
    object
}

/// Install strict/sloppy getter and setter probes that record their receiver
/// in globals, so primitive `this` boxing can be observed.
pub(crate) fn install_primitive_accessors(
    runtime: &Runtime,
    context: &mut Context,
    global: &ObjectRef,
    prototype: &ObjectRef,
) {
    for name in [
        "strictGetThis",
        "sloppyGetThis",
        "strictSetThis",
        "sloppySetThis",
    ] {
        define_global_key(runtime, global, name, Value::Undefined);
    }
    let strict_get = eval_callable(
        runtime,
        context,
        r#"(function() { "use strict"; strictGetThis = this; return this; })"#,
    );
    let sloppy_get = eval_callable(
        runtime,
        context,
        "(function() { sloppyGetThis = this; return this.valueOf(); })",
    );
    let strict_set = eval_callable(
        runtime,
        context,
        r#"(function() { "use strict"; strictSetThis = this; })"#,
    );
    let sloppy_set = eval_callable(runtime, context, "(function() { sloppySetThis = this; })");
    define_accessor(runtime, prototype, "__strictGet", Some(strict_get), None);
    define_accessor(runtime, prototype, "__sloppyGet", Some(sloppy_get), None);
    define_accessor(runtime, prototype, "__strictSet", None, Some(strict_set));
    define_accessor(runtime, prototype, "__sloppySet", None, Some(sloppy_set));
}
