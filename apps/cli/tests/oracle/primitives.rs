use crate::runtime_observation::host_runtime_context;
use std::ffi::OsStr;
use std::io::Write;

use std::process::{Command, Stdio};

use quickjs_oxide::engine::api::{Runtime, RuntimeError, Value};

#[test]
fn compiler_call_capacity_matches_quickjs_error_classes() {
    let oracle = require_qjs_oracle!("compiler-capacity oracle differential");

    let arguments = std::iter::repeat_n("0", usize::from(u16::MAX))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("(function() {{}})({arguments})");

    let (runtime, mut context) = host_runtime_context();
    assert_eq!(context.compile(&source), Err(RuntimeError::Exception));
    let rust_observation = take_rust_error_observation(&runtime, &mut context);
    assert_eq!(rust_observation, "InternalError|stack overflow");

    let oracle_observation = run_oracle_error(&oracle, &source, "65,535 call arguments");
    assert!(
        oracle_observation.starts_with("InternalError|stack overflow"),
        "QuickJS call-stack boundary changed: {oracle_observation:?}"
    );

    let mut too_many = source;
    let closing_parenthesis = too_many
        .rfind(')')
        .expect("generated call expression has a closing parenthesis");
    too_many.insert_str(closing_parenthesis, ",0");
    assert_eq!(context.compile(&too_many), Err(RuntimeError::Exception));
    assert_eq!(
        take_rust_error_observation(&runtime, &mut context),
        "SyntaxError|Too many call arguments"
    );
    assert_eq!(
        run_oracle_error(&oracle, &too_many, "65,536 call arguments"),
        "SyntaxError|Too many call arguments"
    );
}

fn run_oracle_error(oracle: &OsStr, source: &str, description: &str) -> String {
    let quoted = source
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r");
    let script = format!(
        "try {{ eval(\"{quoted}\"); print(\"<no error>\"); }} catch (error) {{ print(error.name + \"|\" + error.message); }}"
    );
    // The call-capacity boundary corpus is larger than Linux ARG_MAX. Feed the
    // exact same script through QuickJS's standard input instead of argv.
    let mut child = Command::new(oracle)
        .args(["--std", "-e", "std.evalScript(std.in.readAsString())"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|error| {
            panic!("could not execute QJS_ORACLE for {description:?} ({source:?}): {error}")
        });
    child
        .stdin
        .take()
        .expect("QJS_ORACLE stdin was not piped")
        .write_all(script.as_bytes())
        .unwrap_or_else(|error| {
            panic!("could not write QJS_ORACLE source for {description:?}: {error}")
        });
    let output = child.wait_with_output().unwrap_or_else(|error| {
        panic!("could not wait for QJS_ORACLE for {description:?}: {error}")
    });
    assert!(
        output.status.success(),
        "QJS_ORACLE failed for {description:?} ({source:?}): {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("QJS_ORACLE emitted non-UTF-8 error output")
        .trim_end()
        .to_owned()
}

fn take_rust_error_observation(
    runtime: &Runtime,
    context: &mut quickjs_oxide::engine::api::Context,
) -> String {
    let Value::Object(error) = context.take_exception().unwrap().unwrap() else {
        panic!("Rust did not materialize a native Error object");
    };
    let name_key = runtime.intern_property_key("name").unwrap();
    let message_key = runtime.intern_property_key("message").unwrap();
    let Value::String(name) = context.get_property(&error, &name_key).unwrap() else {
        panic!("Rust Error name was not a string");
    };
    let Value::String(message) = context.get_property(&error, &message_key).unwrap() else {
        panic!("Rust Error message was not a string");
    };
    format!("{}|{}", name.to_utf8_lossy(), message.to_utf8_lossy())
}
