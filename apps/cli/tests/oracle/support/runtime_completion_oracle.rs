use crate::object_graph_observation::error_string_property as error_string_property_2;
use crate::oracle_cases::CompileCase;
use crate::oracle_cases::GroupedCase as Case;
use crate::oracle_cases::StartMatchCase as MatchCase;
use crate::quickjs_argv_completion_oracle::observe_completion_argv_strip_one_lf;
use crate::quickjs_argv_completion_oracle::observe_completion_argv_trim_end;
use crate::quickjs_array_completion_oracle::observe_array_completion;
use crate::runtime_observation::array_value_text;
use crate::runtime_observation::error_name;
use crate::runtime_observation::normalize_rust_value_text as normalize_rust_value;
use crate::runtime_observation::take_rust_error;
use crate::runtime_observation::{
    checked_value_type, error_string_property, plain_value_type, primitive_value_text,
    primitive_value_text_with_rust_float, string_property, string_property_with_read_context,
};
use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_oracle::run_cli;
use crate::runtime_oracle::{error_string_property as runtime_error_string_property, value_type};
use quickjs_oxide::engine::api::JsString;
use quickjs_oxide::engine::api::{Context, ObjectRef, Runtime, RuntimeError, Value};
use quickjs_oxide::regexp::CompileErrorKind;
use quickjs_oxide::regexp::compile;
use quickjs_oxide::regexp::execute;
use std::ffi::OsStr;
use std::fmt::Write as _;
use std::process::Command;
use std::process::Output;

#[derive(Clone, Copy)]
pub(crate) enum ErrorPropertyStyle {
    Compact,
    ReadContext,
    CaseContext,
    RuntimeCaseContext,
}

/// Observe the public runtime's completion protocol without adding a
/// domain-specific source prelude.
pub(crate) fn observe_eval_completion(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    observe_eval_completion_with(
        runtime,
        context,
        source,
        description,
        value_type,
        primitive_value_text,
        ErrorPropertyStyle::Compact,
    )
}

/// Observe a completion whose Error-property failures include the case name.
pub(crate) fn observe_eval_completion_with_error_context(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    observe_eval_completion_with(
        runtime,
        context,
        source,
        description,
        value_type,
        primitive_value_text,
        ErrorPropertyStyle::CaseContext,
    )
}

/// Observe a legacy vector which intentionally uses Rust float spelling and
/// the older shared Error-property diagnostic contract.
pub(crate) fn observe_legacy_float_eval_completion(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    observe_eval_completion_with(
        runtime,
        context,
        source,
        description,
        value_type,
        primitive_value_text_with_rust_float,
        ErrorPropertyStyle::RuntimeCaseContext,
    )
}

/// Observe a legacy vector while retaining its explicit callable-inspection
/// assertion and Rust float spelling.
pub(crate) fn observe_checked_legacy_float_eval_completion(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    observe_eval_completion_with(
        runtime,
        context,
        source,
        description,
        checked_value_type,
        primitive_value_text_with_rust_float,
        ErrorPropertyStyle::RuntimeCaseContext,
    )
}

/// Observe a completion after prepending a domain-owned source prelude. The
/// caller keeps ownership of the prelude and its value spelling.
pub(crate) fn observe_read_context_eval_completion_with_prelude(
    runtime: &Runtime,
    context: &mut Context,
    prelude: &str,
    source: &str,
    description: &str,
) -> String {
    let source = source_with_prelude(prelude, source);
    observe_eval_completion_with(
        runtime,
        context,
        &source,
        description,
        value_type,
        primitive_value_text,
        ErrorPropertyStyle::ReadContext,
    )
}

#[allow(clippy::too_many_arguments)]
fn observe_eval_completion_with(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
    value_type: fn(&Runtime, &Value) -> &'static str,
    value_text: fn(Value) -> String,
    error_property_style: ErrorPropertyStyle,
) -> String {
    match context.eval(source) {
        Ok(value) => format!(
            "return|{}|{}",
            value_type(runtime, &value),
            value_text(value),
        ),
        Err(RuntimeError::Exception) => {
            let exception = context
                .take_exception()
                .unwrap_or_else(|error| panic!("take Rust exception for {description}: {error}"))
                .unwrap_or_else(|| panic!("Rust exception was missing for {description}"));
            match exception {
                Value::Object(error) => format!(
                    "throw|object|{}|{}",
                    observed_error_string_property(
                        runtime,
                        context,
                        &error,
                        "name",
                        description,
                        error_property_style,
                    ),
                    observed_error_string_property(
                        runtime,
                        context,
                        &error,
                        "message",
                        description,
                        error_property_style,
                    ),
                ),
                value => format!(
                    "throw|{}|{}",
                    value_type(runtime, &value),
                    value_text(value),
                ),
            }
        }
        Err(error) => panic!("Rust engine failure for {description} ({source:?}): {error}"),
    }
}

fn observed_error_string_property(
    runtime: &Runtime,
    context: &mut Context,
    error: &ObjectRef,
    name: &str,
    description: &str,
    style: ErrorPropertyStyle,
) -> String {
    match style {
        ErrorPropertyStyle::Compact => string_property(runtime, context, error, name),
        ErrorPropertyStyle::ReadContext => {
            string_property_with_read_context(runtime, context, error, name)
        }
        ErrorPropertyStyle::CaseContext => {
            error_string_property(runtime, context, error, name, description)
        }
        ErrorPropertyStyle::RuntimeCaseContext => {
            runtime_error_string_property(runtime, context, error, name, description)
        }
    }
}

fn source_with_prelude(prelude: &str, source: &str) -> String {
    format!("{prelude}\n{source}")
}

/// Exercise the shared observation contracts from an existing oracle test so
/// helper extraction cannot silently change every downstream vector at once.
pub(crate) fn assert_runtime_completion_helper_contracts() {
    assert_eq!(primitive_value_text(Value::Float(f64::NAN)), "NaN");
    assert_eq!(primitive_value_text(Value::Float(-0.0)), "0");
    assert_eq!(
        primitive_value_text(Value::Float(f64::INFINITY)),
        "Infinity"
    );
    assert_eq!(primitive_value_text(Value::Float(1.5)), "1.5");
    assert_eq!(
        primitive_value_text_with_rust_float(Value::Float(-0.0)),
        "-0"
    );

    let (runtime, mut context) = host_runtime_context();
    let callable = context.eval("(function(){})").unwrap();
    let ordinary = context.eval("({})").unwrap();
    assert_eq!(checked_value_type(&runtime, &callable), "function");
    assert_eq!(checked_value_type(&runtime, &ordinary), "object");
    assert_eq!(plain_value_type(&callable), "object");

    assert_eq!(
        observe_eval_completion(
            &runtime,
            &mut context,
            "throw 42",
            "thrown primitive canary"
        ),
        "throw|number|42"
    );
    assert_eq!(
        observe_eval_completion(
            &runtime,
            &mut context,
            "throw new Error('boom')",
            "Error object canary",
        ),
        "throw|object|Error|boom"
    );

    let joined = source_with_prelude("var marker = 1;", "marker + 41");
    assert_eq!(joined, "var marker = 1;\nmarker + 41");
    assert_eq!(
        observe_read_context_eval_completion_with_prelude(
            &runtime,
            &mut context,
            "var marker = 1;",
            "marker + 41",
            "prelude identity canary",
        ),
        "return|number|42"
    );
}

/// Compare prefixed vectors while collecting every completion mismatch in the
/// group before failing.
pub(crate) fn compare_read_context_eval_completion_cases_with_prelude(
    prelude: &str,
    group: &str,
    cases: &[(&str, &str)],
) {
    let oracle = require_qjs_oracle!("{group}");
    let mut failures = Vec::new();
    for &(description, original_source) in cases {
        let (runtime, mut context) = host_runtime_context();
        let source = source_with_prelude(prelude, original_source);
        let actual = observe_eval_completion_with(
            &runtime,
            &mut context,
            &source,
            description,
            value_type,
            primitive_value_text,
            ErrorPropertyStyle::ReadContext,
        );
        let expected = crate::quickjs_oracle::observe_completion(&oracle, &source, description);
        if actual != expected {
            failures.push(format!(
                "{description}\nsource: {original_source:?}\noxide: {actual:?}\noracle: {expected:?}",
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{group} drifted in {} case(s):\n\n{}",
        failures.len(),
        failures.join("\n\n"),
    );
}

// Helpers shared verbatim by several oracle modules.

pub(crate) fn compare_argv_cases(group: &str, cases: &[(&str, &str)]) {
    let oracle = require_qjs_oracle!("{group} differential");
    for &(description, source) in cases {
        let (runtime, mut context) = host_runtime_context();
        assert_eq!(
            observe_legacy_float_eval_completion(&runtime, &mut context, source, description),
            observe_completion_argv_strip_one_lf(&oracle, source, description),
            "{group} drifted for {description}: {source:?}",
        );
    }
}

pub(crate) fn compare_value_cases(group: &str, cases: &[(&str, &str)]) {
    let oracle = require_qjs_oracle!("{group} differential");
    for &(description, source) in cases {
        let expected = observe_completion_argv_trim_end(&oracle, source, description);
        let (runtime, mut context) = host_runtime_context();
        assert_eq!(
            observe_eval_completion_with_error_context(&runtime, &mut context, source, description),
            expected,
            "{group} drifted for {description}: {source:?}",
        );
    }
}

pub(crate) fn compare_cli(oracle: &OsStr, options: &[&str], source: &str, description: &str) {
    let rust = run_cli(
        env!("CARGO_BIN_EXE_qjs").as_ref(),
        options,
        source,
        description,
    );
    let quickjs = run_cli(oracle, options, source, description);
    assert_eq!(rust.status.code(), quickjs.status.code(), "{description}");
    assert_eq!(rust.stdout, quickjs.stdout, "{description}");
    assert_eq!(rust.stderr, quickjs.stderr, "{description}");
}

const SCRIPT_CASE_OBSERVER: &str = r#"
try {
  var value = std.evalScript(scriptArgs[0]);
  print('return|' + typeof value + '|' + String(value));
} catch (error) {
  if (error !== null && typeof error === 'object')
    print('throw|object|' + error.name + '|' + error.message);
  else
    print('throw|' + typeof error + '|' + String(error));
}
"#;

const SCRIPT_CASE_NAME_OBSERVER: &str = r#"
try {
  var value = std.evalScript(scriptArgs[0]);
  print('return|' + typeof value + '|' + String(value));
} catch (error) {
  if (error !== null && typeof error === 'object')
    print('throw|object|' + error.name);
  else
    print('throw|' + typeof error + '|' + String(error));
}
"#;

pub(crate) fn oracle_script_case_observation(oracle: &OsStr, case: &Case) -> String {
    observe_script_case(oracle, SCRIPT_CASE_OBSERVER, case)
}

/// Like [`oracle_script_case_observation`], but a thrown object reports only
/// its `name`.
pub(crate) fn oracle_script_case_name_observation(oracle: &OsStr, case: &Case) -> String {
    observe_script_case(oracle, SCRIPT_CASE_NAME_OBSERVER, case)
}

fn observe_script_case(oracle: &OsStr, wrapper: &str, case: &Case) -> String {
    let output = Command::new(oracle)
        .args(["--std", "-e", wrapper, case.source])
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "could not run QuickJS for {} / {}: {error}",
                case.group, case.description,
            )
        });
    assert!(
        output.status.success(),
        "QuickJS observer failed for {} / {}: {}",
        case.group,
        case.description,
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .unwrap_or_else(|error| {
            panic!(
                "QuickJS output was not UTF-8 for {} / {}: {error}",
                case.group, case.description,
            )
        })
        .trim_end()
        .to_owned()
}

pub(crate) fn run(executable: &OsStr, source: &str) -> Output {
    Command::new(executable)
        .args(["-e", source])
        .output()
        .unwrap_or_else(|error| panic!("could not run {executable:?}: {error}"))
}

pub(crate) fn rust_value_observation(source: &str, description: &str) -> String {
    let runtime = host_runtime();
    let value = runtime
        .new_context()
        .expect("create context")
        .eval(source)
        .unwrap_or_else(|error| panic!("Rust rejected {description} ({source:?}): {error}"));
    normalize_rust_value(value)
}

pub(crate) const ORACLE_NORMALIZER: &str = r#"
var __qjo_type = typeof __qjo_value;
if (__qjo_type === "number") {
    if (__qjo_value !== __qjo_value) {
        print("number|NaN");
    } else if (__qjo_value === 0 && 1 / __qjo_value === -Infinity) {
        print("number|-0");
    } else if (__qjo_value === Infinity) {
        print("number|Infinity");
    } else if (__qjo_value === -Infinity) {
        print("number|-Infinity");
    } else {
        print("number|" + String(__qjo_value));
    }
} else if (__qjo_type === "string") {
    var __qjo_units = "";
    for (var __qjo_index = 0; __qjo_index < __qjo_value.length; __qjo_index++) {
        var __qjo_hex = __qjo_value.charCodeAt(__qjo_index).toString(16);
        if (__qjo_index !== 0) __qjo_units += ",";
        __qjo_units += ("0000" + __qjo_hex).slice(-4);
    }
    print("string|" + __qjo_value.length + "|" + __qjo_units);
} else if (__qjo_value === null) {
    print("object|null");
} else if (__qjo_type === "symbol") {
    print("symbol|<identity>");
} else if (__qjo_type === "object" || __qjo_type === "function") {
    print("object|<identity>");
} else {
    print(__qjo_type + "|" + String(__qjo_value));
}
"#;

pub(crate) fn rust_error_observation(source: &str) -> String {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(context.eval(source), Err(RuntimeError::Exception));
    take_rust_error(&runtime, &mut context)
}

pub(crate) fn observe_rust_with_engine_errors(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    match context.eval(source) {
        Ok(value) => format!(
            "return|{}|{}",
            checked_value_type(runtime, &value),
            primitive_value_text_with_rust_float(value),
        ),
        Err(RuntimeError::Exception) => {
            let exception = context
                .take_exception()
                .unwrap_or_else(|error| panic!("take Rust exception for {description}: {error}"))
                .unwrap_or_else(|| panic!("Rust exception was missing for {description}"));
            match exception {
                Value::Object(error) => format!(
                    "throw|object|{}|{}",
                    runtime_error_string_property(runtime, context, &error, "name", description),
                    runtime_error_string_property(runtime, context, &error, "message", description),
                ),
                value => format!(
                    "throw|{}|{}",
                    checked_value_type(runtime, &value),
                    primitive_value_text_with_rust_float(value),
                ),
            }
        }
        Err(error) => format!("engine|{error}"),
    }
}

pub(crate) fn rust_case_observation(case: &Case) -> String {
    let (runtime, mut context) = host_runtime_context();
    match context.eval(case.source) {
        Ok(value) => format!(
            "return|{}|{}",
            checked_value_type(&runtime, &value),
            primitive_value_text(value),
        ),
        Err(RuntimeError::Exception) => {
            let exception = context
                .take_exception()
                .unwrap_or_else(|error| {
                    panic!(
                        "take Rust exception for {} / {}: {error}",
                        case.group, case.description,
                    )
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Rust exception was missing for {} / {}",
                        case.group, case.description,
                    )
                });
            match exception {
                Value::Object(error) => format!(
                    "throw|object|{}|{}",
                    error_string_property_2(&runtime, &mut context, &error, "name", case),
                    error_string_property_2(&runtime, &mut context, &error, "message", case),
                ),
                value => format!(
                    "throw|{}|{}",
                    checked_value_type(&runtime, &value),
                    primitive_value_text(value),
                ),
            }
        }
        Err(error) => panic!(
            "Rust engine failure for {} / {} ({:?}): {error}",
            case.group, case.description, case.source,
        ),
    }
}

pub(crate) fn compare_array_value_cases(group: &str, cases: &[(&str, &str)]) {
    let oracle = require_qjs_oracle!("{group} differential");
    for &(description, source) in cases {
        let expected = observe_array_completion(&oracle, source, description);
        let (runtime, mut context) = host_runtime_context();
        assert_eq!(
            observe_array_aware_eval(&runtime, &mut context, source, description),
            expected,
            "{group} drifted for {description}: {source:?}",
        );
    }
}

pub(crate) fn observe_array_aware_eval(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    match context.eval(source) {
        Ok(Value::Object(object)) if runtime.is_array_object(&object).unwrap() => format!(
            "return|array|{}",
            array_value_text(runtime, context, &object, description),
        ),
        Ok(value) => format!(
            "return|{}|{}",
            value_type(runtime, &value),
            primitive_value_text(value),
        ),
        Err(RuntimeError::Exception) => {
            let exception = context
                .take_exception()
                .unwrap_or_else(|error| panic!("take Rust exception for {description}: {error}"))
                .unwrap_or_else(|| panic!("Rust exception was missing for {description}"));
            match exception {
                Value::Object(error) => format!(
                    "throw|object|{}|{}",
                    error_string_property(runtime, context, &error, "name", description),
                    error_string_property(runtime, context, &error, "message", description),
                ),
                value => format!(
                    "throw|{}|{}",
                    value_type(runtime, &value),
                    primitive_value_text(value),
                ),
            }
        }
        Err(error) => panic!("Rust engine failure for {description} ({source:?}): {error}"),
    }
}

pub(crate) fn rust_case_name_observation(case: &Case) -> String {
    let (runtime, mut context) = host_runtime_context();
    match context.eval(case.source) {
        Ok(value) => format!(
            "return|{}|{}",
            checked_value_type(&runtime, &value),
            primitive_value_text(value),
        ),
        Err(RuntimeError::Exception) => {
            let exception = context
                .take_exception()
                .unwrap_or_else(|error| {
                    panic!(
                        "take Rust exception for {} / {}: {error}",
                        case.group, case.description,
                    )
                })
                .unwrap_or_else(|| {
                    panic!(
                        "Rust exception was missing for {} / {}",
                        case.group, case.description,
                    )
                });
            match exception {
                Value::Object(error) => format!(
                    "throw|object|{}",
                    error_name(&runtime, &mut context, &error, case),
                ),
                value => format!(
                    "throw|{}|{}",
                    checked_value_type(&runtime, &value),
                    primitive_value_text(value),
                ),
            }
        }
        Err(error) => panic!(
            "Rust engine failure for {} / {} ({:?}): {error}",
            case.group, case.description, case.source,
        ),
    }
}

pub(crate) fn observe_rust(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    match context.eval(source) {
        Ok(value) => format!(
            "return|{}|{}",
            value_type(runtime, &value),
            primitive_value_text_with_rust_float(value),
        ),
        Err(RuntimeError::Exception) => {
            let exception = context
                .take_exception()
                .unwrap_or_else(|error| panic!("take Rust exception for {description}: {error}"))
                .unwrap_or_else(|| panic!("Rust exception was missing for {description}"));
            match exception {
                Value::Object(error) => format!(
                    "throw|object|{}|{}",
                    runtime_error_string_property(runtime, context, &error, "name", description),
                    runtime_error_string_property(runtime, context, &error, "message", description),
                ),
                value => format!(
                    "throw|{}|{}",
                    value_type(runtime, &value),
                    primitive_value_text_with_rust_float(value),
                ),
            }
        }
        Err(error) => panic!("Rust engine failure for {description} ({source:?}): {error}"),
    }
}

pub(crate) fn observe_rust_result(
    result: Option<&quickjs_oxide::regexp::RegExpMatch>,
    input: &[u16],
) -> String {
    let Some(result) = result else {
        return "N".to_owned();
    };
    let complete = result
        .capture(0)
        .expect("a successful match has capture zero");
    let mut output = format!("M|{}|{}", complete.start, result.captures().len());
    for capture in result.captures() {
        match capture {
            None => output.push_str("|U"),
            Some(range) => {
                write!(output, "|{},{}:", range.start, range.end).unwrap();
                for unit in &input[range.clone()] {
                    write!(output, "{unit:04x}").unwrap();
                }
            }
        }
    }
    output
}

pub(crate) const REGEXP_COMPLETION_PRELUDE: &str = r#"
function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+
        __bit(descriptor.configurable);
}
function __isConstructor(value){
    try{Reflect.construct(function(){},[],value);return true}
    catch(_error){return false}
}
function __show(value){
    if(value===undefined)return "undefined";
    if(value===null)return "null";
    if(typeof value==="number"){
        if(value!==value)return "NaN";
        if(value===0)return 1/value===-Infinity?"-0":"+0";
    }
    return String(value);
}
function __completion(callback){
    try{return "return:"+String(callback())}
    catch(error){
        if(error!==null&&typeof error==="object")
            return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
"#;

pub(crate) fn observe_rust_match(case: &MatchCase) -> String {
    let pattern = JsString::try_from_utf16(case.pattern.iter().copied()).unwrap();
    let flags = JsString::try_from_utf8(case.flags).unwrap();
    let program = match compile(&pattern, &flags) {
        Ok(program) => program,
        Err(error) => {
            return format!(
                "CompileError|{:?}|{}|{}",
                error.kind(),
                error.position(),
                error.message(),
            );
        }
    };
    match execute(&program, &case.input, case.start) {
        Ok(result) => observe_rust_result(result.as_ref(), &case.input),
        Err(error) => format!("ExecError|{error}"),
    }
}

pub(crate) fn observe_rust_compile(case: &CompileCase) -> String {
    let pattern = JsString::try_from_utf8(case.pattern).unwrap();
    let flags = JsString::try_from_utf8(case.flags).unwrap();
    match compile(&pattern, &flags) {
        Ok(_) => "ok".to_owned(),
        Err(error) if matches!(error.kind(), CompileErrorKind::Syntax) => "SyntaxError".to_owned(),
        Err(error) => format!("CompileError|{:?}", error.kind()),
    }
}

/// Compare prelude-prefixed completion vectors, collecting every drift before
/// failing. The caller owns the prelude, value spelling and Error reader.
pub(crate) fn compare_prelude_cases_collecting_failures(
    prelude: &str,
    group: &str,
    cases: &[(&str, &str)],
    value_text: fn(Value) -> String,
    error_property_style: ErrorPropertyStyle,
    oracle_completion: fn(&OsStr, &str, &str) -> String,
) {
    let oracle = require_qjs_oracle!("{group}");
    let mut failures = Vec::new();
    for &(description, source) in cases {
        let (runtime, mut context) = host_runtime_context();
        let prefixed = source_with_prelude(prelude, source);
        let actual = observe_eval_completion_with(
            &runtime,
            &mut context,
            &prefixed,
            description,
            value_type,
            value_text,
            error_property_style,
        );
        let expected = oracle_completion(&oracle, &prefixed, description);
        if actual != expected {
            failures.push(format!(
                "{description}\nsource: {source:?}\noxide: {actual:?}\noracle: {expected:?}",
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{group} drifted in {} case(s):\n\n{}",
        failures.len(),
        failures.join("\n\n"),
    );
}

pub(crate) const REGEXP_REPLACE_PRELUDE: &str = r#"
function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+
        __bit(descriptor.configurable);
}
function __isConstructor(value){
    try{Reflect.construct(function(){},[],value);return true}
    catch(_error){return false}
}
function __show(value){
    if(value===undefined)return "undefined";
    if(value===null)return "null";
    if(typeof value==="number"){
        if(value!==value)return "NaN";
        if(value===0)return 1/value===-Infinity?"-0":"+0";
    }
    return String(value);
}
function __completion(callback){
    try{return "return:"+String(callback())}
    catch(error){
        if(error!==null&&typeof error==="object")
            return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
function __units(value){
    var string=String(value),output=[],index=0;
    while(index<string.length){
        output[index]=string.charCodeAt(index).toString(16);
        index++;
    }
    return string.length+"["+output.join(",")+"]";
}
"#;
