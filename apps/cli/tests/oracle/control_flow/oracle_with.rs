//! Differential coverage for `with` statement semantics.

use crate::runtime_observation::host_runtime_context;

use quickjs_oxide::engine::api::{JsString, Value};
use std::ffi::OsStr;
use std::process::Command;

#[test]
fn with_global_reference_sees_a_const_from_an_earlier_script() {
    let declaration = "const __qjo_with_prior_const = 1; var __qjo_with_prior_side = 0";
    let assignment = r#"
        var __qjo_with_prior_error = "none";
        try {
            with ({}) {
                __qjo_with_prior_const = (__qjo_with_prior_side = 1, 2);
            }
        } catch (error) {
            __qjo_with_prior_error = error.name;
        }
        __qjo_with_prior_error + "|" + __qjo_with_prior_side
    "#;

    let (_runtime, mut context) = host_runtime_context();
    drop(context.eval(declaration).unwrap());
    assert_eq!(
        context.eval(assignment).unwrap(),
        Value::String(JsString::try_from_utf8("TypeError|0").unwrap())
    );

    let Some(oracle) = std::env::var_os("QJS_ORACLE") else {
        return;
    };
    assert_eq!(
        oracle_sequence_observation(&oracle, &[declaration, assignment]),
        "return|string|TypeError|0"
    );
}

#[test]
fn with_global_reference_observes_a_lexical_declared_after_function_publication() {
    let function = r#"
        var __qjo_with_late_const_side = 0;
        var __qjo_with_late_const_function;
        with ({}) {
            __qjo_with_late_const_function = function () {
                __qjo_with_late_const = (__qjo_with_late_const_side = 1, 2);
            };
        }
    "#;
    let declaration = "const __qjo_with_late_const = 1";
    let observation = r#"
        var __qjo_with_late_const_error = "none";
        try { __qjo_with_late_const_function(); }
        catch (error) { __qjo_with_late_const_error = error.name; }
        __qjo_with_late_const_error + "|" + __qjo_with_late_const_side
    "#;

    let (_runtime, mut context) = host_runtime_context();
    drop(context.eval(function).unwrap());
    drop(context.eval(declaration).unwrap());
    assert_eq!(
        context.eval(observation).unwrap(),
        Value::String(JsString::try_from_utf8("TypeError|0").unwrap())
    );

    let Some(oracle) = std::env::var_os("QJS_ORACLE") else {
        return;
    };
    assert_eq!(
        oracle_sequence_observation(&oracle, &[function, declaration, observation]),
        "return|string|TypeError|0"
    );
}

fn oracle_sequence_observation(oracle: &OsStr, scripts: &[&str]) -> String {
    assert!(
        !scripts.is_empty(),
        "QuickJS sequence needs an observation script"
    );
    let wrapper = r#"
for (var index = 0; index + 1 < scriptArgs.length; index++)
  std.evalScript(scriptArgs[index]);
try {
  var value = std.evalScript(scriptArgs[scriptArgs.length - 1]);
  print('return|' + typeof value + '|' + String(value));
} catch (error) {
  if (error !== null && typeof error === 'object')
    print('throw|object|' + error.name + '|' + error.message);
  else
    print('throw|' + typeof error + '|' + String(error));
}
"#;
    let mut command = Command::new(oracle);
    command.args(["--std", "-e", wrapper]).args(scripts);
    let output = command
        .output()
        .expect("could not run QuickJS global-reference sequence");
    assert!(
        output.status.success(),
        "QuickJS global-reference observer failed: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .expect("QuickJS global-reference output was not UTF-8")
        .trim_end()
        .to_owned()
}
