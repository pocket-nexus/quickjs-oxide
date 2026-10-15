use quickjs_oxide::engine::api::JsString;
use quickjs_oxide::engine::api::RuntimeError;
use quickjs_oxide::engine::api::number_to_string;
use quickjs_oxide::engine::api::{CallableRef, Context, ObjectRef, Runtime, Value};
use std::fmt::Write as _;

/// Create a runtime with the system host services and one fresh context.
/// Destructure it as `let (runtime, mut context)` so the context still drops
/// before its runtime.
pub(crate) fn host_runtime_context() -> (Runtime, Context) {
    let runtime = host_runtime();
    let context = runtime.new_context().expect("create context");
    (runtime, context)
}

/// Create a runtime with the system host services.
pub(crate) fn host_runtime() -> Runtime {
    Runtime::new_with_host_services(quickjs_oxide_host::SystemHostServices::default())
}

/// Classify a value for completion-vector output while retaining the explicit
/// callable-inspection assertion used by the older oracle helpers.
pub(crate) fn checked_value_type(runtime: &Runtime, value: &Value) -> &'static str {
    match value {
        Value::Undefined => "undefined",
        Value::Null => "object",
        Value::Bool(_) => "boolean",
        Value::Int(_) | Value::Float(_) => "number",
        Value::BigInt(_) => "bigint",
        Value::String(_) => "string",
        Value::Object(object) => {
            if runtime
                .as_callable(object)
                .expect("inspect callable")
                .is_some()
            {
                "function"
            } else {
                "object"
            }
        }
        Value::Symbol(_) => "symbol",
    }
}

/// Render an observed completion value using ECMAScript number formatting.
pub(crate) fn primitive_value_text(value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => quickjs_oxide::engine::api::number_to_string(value),
        Value::BigInt(value) => value.to_string(),
        Value::String(value) => value.to_utf8_lossy(),
        Value::Object(_) => "<object>".to_owned(),
        Value::Symbol(_) => "<symbol>".to_owned(),
    }
}

/// Render a legacy completion vector which intentionally uses Rust's float
/// formatter. Keeping this separate preserves those vectors byte-for-byte.
pub(crate) fn primitive_value_text_with_rust_float(value: Value) -> String {
    match value {
        Value::Undefined => "undefined".to_owned(),
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::Float(value) => value.to_string(),
        Value::BigInt(value) => value.to_string(),
        Value::String(value) => value.to_utf8_lossy(),
        Value::Object(_) => "<object>".to_owned(),
        Value::Symbol(_) => "<symbol>".to_owned(),
    }
}

/// Read an object property and require it to be callable.
pub(crate) fn property_callable(
    runtime: &Runtime,
    context: &mut Context,
    owner: &ObjectRef,
    name: &str,
) -> CallableRef {
    let Value::Object(object) = context
        .get_property(owner, &runtime.intern_property_key(name).unwrap())
        .unwrap()
    else {
        panic!("{name} was not an object");
    };
    runtime
        .as_callable(&object)
        .unwrap()
        .unwrap_or_else(|| panic!("{name} was not callable"))
}

/// Read a callable property while retaining the contextual read failure used
/// by oracle vectors which diagnose setup failures.
pub(crate) fn property_callable_with_read_context(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> CallableRef {
    let key = runtime.intern_property_key(name).unwrap();
    let Value::Object(function) = context
        .get_property(object, &key)
        .unwrap_or_else(|error| panic!("read callable {name}: {error}"))
    else {
        panic!("{name} was not an object");
    };
    runtime
        .as_callable(&function)
        .unwrap()
        .unwrap_or_else(|| panic!("{name} was not callable"))
}

/// Read a string-valued property using the compact completion-vector failure
/// contract.
pub(crate) fn string_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> String {
    let Value::String(value) = context
        .get_property(object, &runtime.intern_property_key(name).unwrap())
        .unwrap()
    else {
        panic!("{name} was not a String property");
    };
    value.to_utf8_lossy()
}

/// Read a string-valued property while preserving contextual setup failures.
pub(crate) fn string_property_with_read_context(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> String {
    let key = runtime.intern_property_key(name).unwrap();
    let Value::String(value) = context
        .get_property(object, &key)
        .unwrap_or_else(|error| panic!("read string property {name}: {error}"))
    else {
        panic!("{name} was not a string");
    };
    value.to_utf8_lossy()
}

/// Read an Error string property while retaining the case description in
/// setup failures.
pub(crate) fn error_string_property(
    runtime: &Runtime,
    context: &mut Context,
    error: &ObjectRef,
    name: &str,
    description: &str,
) -> String {
    let key = runtime.intern_property_key(name).unwrap();
    let Value::String(value) = context
        .get_property(error, &key)
        .unwrap_or_else(|failure| panic!("read Error.{name} for {description}: {failure}"))
    else {
        panic!("Error.{name} was not a string for {description}");
    };
    value.to_utf8_lossy()
}

/// Take a described pending exception and require an object value.
pub(crate) fn take_exception_object(context: &mut Context, description: &str) -> ObjectRef {
    let Value::Object(error) = context
        .take_exception()
        .unwrap_or_else(|failure| panic!("take {description}: {failure}"))
        .unwrap_or_else(|| panic!("{description} was missing"))
    else {
        panic!("{description} was not an object");
    };
    error
}

/// Take the compact, undescribed pending exception used by most object and
/// string intrinsic vectors.
pub(crate) fn take_pending_exception_object(context: &mut Context) -> ObjectRef {
    let Some(Value::Object(error)) = context.take_exception().unwrap() else {
        panic!("pending exception was not an object");
    };
    error
}

/// Take a pending exception expected to be an Error object.
pub(crate) fn take_error_object(context: &mut Context) -> ObjectRef {
    let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
        panic!("operation did not throw an Error object");
    };
    error
}

/// Take a pending exception expected to be an arbitrary object.
pub(crate) fn take_thrown_object(context: &mut Context) -> ObjectRef {
    let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
        panic!("operation did not throw an object");
    };
    error
}

// Helpers shared verbatim by several oracle modules.

pub(crate) fn string_units(value: &JsString) -> String {
    value
        .utf16_units()
        .map(|unit| format!("{unit:04x}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn eval(context: &mut Context, source: &str) -> Value {
    context.eval(source).unwrap_or_else(|error| {
        assert!(
            error != RuntimeError::Exception,
            "unexpected JavaScript exception: {:?}",
            context.take_exception()
        );
        panic!("unexpected engine error: {error}");
    })
}

pub(crate) fn text(value: Value) -> String {
    let Value::String(value) = value else {
        panic!("expected a string");
    };
    value.to_utf8_lossy()
}

pub(crate) fn integer(value: Value) -> i32 {
    let Value::Int(value) = value else {
        panic!("expected an integer");
    };
    value
}

pub(crate) fn drain(runtime: &Runtime) -> usize {
    let mut count = 0;
    while runtime.is_job_pending().expect("runtime state") {
        assert!(runtime.execute_pending_job().unwrap().executed());
        count += 1;
    }
    count
}

pub(crate) fn normalize_rust_value_text(value: Value) -> String {
    match value {
        Value::Undefined => "undefined|undefined".to_owned(),
        Value::Null => "object|null".to_owned(),
        Value::Bool(value) => format!("boolean|{value}"),
        Value::Int(value) => format!("number|{value}"),
        Value::Float(value) => format!("number|{value}"),
        Value::BigInt(value) => format!("bigint|{value}"),
        Value::String(value) => format!("string|{}", value.to_utf8_lossy()),
        Value::Object(_) => "object|<object>".to_owned(),
        Value::Symbol(_) => "symbol|<symbol>".to_owned(),
    }
}

pub(crate) fn normalize_rust_value_units(value: &Value) -> String {
    match value {
        Value::Undefined => "undefined|undefined".to_owned(),
        Value::Null => "object|null".to_owned(),
        Value::Bool(value) => format!("boolean|{value}"),
        Value::Int(value) => normalize_number(f64::from(*value)),
        Value::Float(value) => normalize_number(*value),
        Value::BigInt(value) => format!("bigint|{value}"),
        Value::String(value) => {
            let units = value
                .utf16_units()
                .map(|unit| format!("{unit:04x}"))
                .collect::<Vec<_>>()
                .join(",");
            format!("string|{}|{units}", value.len())
        }
        Value::Symbol(_) => "symbol|<identity>".to_owned(),
        Value::Object(_) => "object|<identity>".to_owned(),
    }
}

#[allow(clippy::float_cmp)]
pub(crate) fn normalize_number(value: f64) -> String {
    if value.is_nan() {
        "number|NaN".to_owned()
    } else if value == 0.0 && value.is_sign_negative() {
        "number|-0".to_owned()
    } else if value == f64::INFINITY {
        "number|Infinity".to_owned()
    } else if value == f64::NEG_INFINITY {
        "number|-Infinity".to_owned()
    } else {
        format!("number|{}", number_to_string(value))
    }
}

pub(crate) fn string_value(value: &str) -> Value {
    Value::String(JsString::try_from_utf8(value).unwrap())
}

pub(crate) fn show_exception(runtime: &Runtime, context: &mut Context, exception: Value) -> String {
    match exception {
        Value::String(value) => format!("throw-string:{}", value.to_utf8_lossy()),
        Value::Object(error) if runtime.is_error_object(&error).unwrap() => {
            let name = runtime.intern_property_key("name").unwrap();
            let message = runtime.intern_property_key("message").unwrap();
            let Value::String(name) = context.get_property(&error, &name).unwrap() else {
                panic!("error name was not a string");
            };
            let Value::String(message) = context.get_property(&error, &message).unwrap() else {
                panic!("error message was not a string");
            };
            format!("throw:{}|{}", name.to_utf8_lossy(), message.to_utf8_lossy())
        }
        value => panic!("unexpected thrown probe value: {value:?}"),
    }
}

pub(crate) fn error_text(
    runtime: &Runtime,
    context: &mut Context,
    error: &ObjectRef,
    name: &str,
) -> String {
    let Value::String(value) = context
        .get_property(error, &runtime.intern_property_key(name).unwrap())
        .unwrap()
    else {
        panic!("Error.{name} was not a string");
    };
    value.to_utf8_lossy()
}

pub(crate) fn array_value_text(
    runtime: &Runtime,
    context: &mut Context,
    array: &ObjectRef,
    description: &str,
) -> String {
    let length_key = runtime.intern_property_key("length").unwrap();
    let length = context
        .get_property(array, &length_key)
        .unwrap_or_else(|error| panic!("read result Array length for {description}: {error}"));
    let length = match length {
        Value::Int(value) if value >= 0 => value as usize,
        Value::Float(value) if value >= 0.0 && value.fract() == 0.0 => value as usize,
        value => panic!(
            "result Array length for {description} was invalid: {}",
            primitive_value_text(value),
        ),
    };
    (0..length)
        .map(|index| {
            let key = runtime.intern_property_key(&index.to_string()).unwrap();
            let value = context.get_property(array, &key).unwrap_or_else(|error| {
                panic!("read result Array[{index}] for {description}: {error}")
            });
            primitive_value_text(value)
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(crate) fn hex_bytes(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().collect()
}

pub(crate) fn js_utf16(units: &[u16]) -> String {
    let mut source = String::from("\"");
    for unit in units {
        write!(source, "\\u{unit:04x}").unwrap();
    }
    source.push('"');
    source
}

pub(crate) fn assert_native_error(
    runtime: &Runtime,
    context: &mut Context,
    method: &CallableRef,
    this_value: Value,
    arguments: &[Value],
    expected_prototype: &ObjectRef,
) {
    assert_eq!(
        context.call(method, this_value, arguments),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_pending_exception_object(context))
            .unwrap()
            .as_ref(),
        Some(expected_prototype),
    );
}

pub(crate) fn error_string(
    runtime: &Runtime,
    context: &mut Context,
    error: &ObjectRef,
    name: &str,
) -> JsString {
    let Value::String(value) = context
        .get_property(error, &runtime.intern_property_key(name).unwrap())
        .unwrap()
    else {
        panic!("Error.{name} was not a String");
    };
    value
}

#[cfg(feature = "test262-host")]
pub(crate) fn string_text(value: Value) -> String {
    let Value::String(value) = value else {
        panic!("expected a string value");
    };
    value.to_utf8_lossy()
}

/// Spell UTF-16 code units as a JavaScript `String.fromCharCode(...)` call.
pub(crate) fn from_char_code_source(units: &[u16]) -> String {
    let values = units
        .iter()
        .map(|unit| format!("0x{unit:04x}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("String.fromCharCode({values})")
}
