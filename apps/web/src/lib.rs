use js_sys::{Object, Reflect};
use quickjs_oxide::engine::api::core::{
    CompleteOrdinaryPropertyDescriptor, Runtime, RuntimeError, RuntimeOptions, Value,
};
use quickjs_oxide::engine::api::number_to_string;
use quickjs_oxide::{QUICKJS_COMPAT_VERSION, QUICKJS_OXIDE_VERSION};
use quickjs_oxide_web_host::WebHost;

use wasm_bindgen::prelude::*;

const PLAYGROUND_FILENAME: &str = "<playground>";
const WEB_CAN_BLOCK: bool = false;
/// Nested re-entries the engine may make. A WebAssembly host's machine stack
/// is the JavaScript engine's, which the core's byte measure (of the
/// linear-memory stack) cannot see, so the playground bounds the depth.
const WEB_NATIVE_DEPTH_LIMIT: u32 = 600;
/// Half of the linear-memory stack `wasm-ld` reserves (1 MiB), so a native
/// frame never runs past it before the check trips.
const WEB_NATIVE_STACK_BUDGET: usize = 512 << 10;

const BUILD_COMMIT: &str = match option_env!("QUICKJS_OXIDE_COMMIT") {
    Some(commit) => commit,
    None => "local",
};

struct EvalResult {
    ok: bool,
    kind: &'static str,
    text: String,
}

impl EvalResult {
    fn success(runtime: &Runtime, value: &Value) -> Self {
        let (kind, text) = value_text(runtime, value);
        Self {
            ok: true,
            kind,
            text,
        }
    }

    fn engine_error(error: &RuntimeError) -> Self {
        Self {
            ok: false,
            kind: "engine-error",
            text: error.to_string(),
        }
    }

    fn exception(runtime: &Runtime, value: &Value) -> Self {
        Self {
            ok: false,
            kind: "exception",
            text: exception_text(runtime, value),
        }
    }

    fn into_js(self) -> JsValue {
        let object = Object::new();
        set_result_field(&object, "ok", &JsValue::from_bool(self.ok));
        set_result_field(&object, "kind", &JsValue::from_str(self.kind));
        set_result_field(&object, "text", &JsValue::from_str(&self.text));
        object.into()
    }
}

/// Evaluate one isolated script with quickjs-oxide's Rust compiler and runtime.
///
/// A fresh runtime and realm are used for every call. The completion is returned
/// as a plain JavaScript object with stable `ok`, `kind`, and `text` fields.
#[wasm_bindgen]
pub fn evaluate(source: &str) -> JsValue {
    evaluate_with_engine(source).into_js()
}

/// Return provenance and host-policy metadata for this exact WebAssembly build.
#[wasm_bindgen]
pub fn engine_metadata() -> JsValue {
    let object = Object::new();
    set_result_field(&object, "engine", &JsValue::from_str("quickjs-oxide"));
    set_result_field(
        &object,
        "crateVersion",
        &JsValue::from_str(QUICKJS_OXIDE_VERSION),
    );
    set_result_field(
        &object,
        "quickjsTarget",
        &JsValue::from_str(&format!("QuickJS {QUICKJS_COMPAT_VERSION}")),
    );
    set_result_field(&object, "buildCommit", &JsValue::from_str(BUILD_COMMIT));
    set_result_field(&object, "canBlock", &JsValue::from_bool(WEB_CAN_BLOCK));
    object.into()
}

fn evaluate_with_engine(source: &str) -> EvalResult {
    let runtime = Runtime::with_host(
        WebHost,
        RuntimeOptions {
            native_depth_limit: WEB_NATIVE_DEPTH_LIMIT,
            native_stack_budget: WEB_NATIVE_STACK_BUDGET,
            ..RuntimeOptions::default()
        },
    );
    if let Err(error) = runtime.set_can_block(WEB_CAN_BLOCK) {
        return EvalResult::engine_error(&error);
    }
    let mut context = runtime.context();
    let value = match context.eval_with_filename(source, PLAYGROUND_FILENAME) {
        Ok(value) => value,
        Err(RuntimeError::Exception(exception)) => {
            return EvalResult::exception(&runtime, &exception);
        }
        Err(error) => return EvalResult::engine_error(&error),
    };

    loop {
        match runtime.run_pending_job() {
            Ok(outcome) if outcome.executed() => {}
            Ok(_) => break,
            Err(RuntimeError::Exception(exception)) => {
                return EvalResult::exception(&runtime, &exception);
            }
            Err(error) => return EvalResult::engine_error(&error),
        }
    }

    EvalResult::success(&runtime, &value)
}

fn value_text(runtime: &Runtime, value: &Value) -> (&'static str, String) {
    match value {
        Value::Undefined => ("undefined", "undefined".to_owned()),
        Value::Null => ("null", "null".to_owned()),
        Value::Bool(value) => ("boolean", value.to_string()),
        Value::Int(value) => ("number", value.to_string()),
        Value::Float(value) => ("number", number_to_string(*value)),
        Value::BigInt(value) => ("bigint", format!("{value}n")),
        Value::String(value) => ("string", value.to_utf8_lossy()),
        Value::Object(_) => ("object", "[object Object]".to_owned()),
        Value::Symbol(symbol) => {
            let description = runtime
                .symbol_description(symbol)
                .ok()
                .flatten()
                .map_or_else(String::new, |value| value.to_utf8_lossy());
            ("symbol", format!("Symbol({description})"))
        }
    }
}

/// An error as `name: message` (QuickJS's printer without the backtrace);
/// any other thrown value as a completion.
fn exception_text(runtime: &Runtime, value: &Value) -> String {
    if let Value::Object(object) = value
        && runtime.is_error_object(object).unwrap_or(false)
        && let Ok(printed) = runtime.print_value(value)
    {
        let stack = runtime
            .intern_property_key("stack")
            .and_then(|key| runtime.get_own_property(object, &key))
            .ok()
            .flatten();
        if let Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::String(stack),
            ..
        }) = stack
        {
            let stack = stack.to_utf8_lossy();
            let suffix = format!("\n{}", stack.strip_suffix('\n').unwrap_or(&stack));
            if let Some(head) = printed.strip_suffix(&suffix) {
                return head.to_owned();
            }
        }
        return printed;
    }
    value_text(runtime, value).1
}

fn set_result_field(object: &Object, key: &str, value: &JsValue) {
    Reflect::set(object, &JsValue::from_str(key), value)
        .expect("setting a fresh result object's data property must succeed");
}
