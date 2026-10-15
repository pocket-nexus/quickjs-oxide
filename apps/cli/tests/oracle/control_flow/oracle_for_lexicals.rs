//! Differential coverage for classic `for` lexical environments.

use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::normalize_rust_value_text as normalize_rust_value;
use std::ffi::OsStr;
use std::process::Command;

#[test]
fn script_for_capture_survives_a_following_eval_like_pinned_quickjs() {
    let oracle = require_qjs_oracle!("lexical-for cross-eval differential");
    let setup = "Function.saved=undefined;for(let value=0;value<1;value++){Function.saved=function(){return value};}";
    let observation = "Function.saved()+'|'+typeof value";

    let (_runtime, mut context) = host_runtime_context();
    drop(
        context
            .eval(setup)
            .unwrap_or_else(|error| panic!("Rust rejected lexical-for setup: {error}")),
    );
    let value = context
        .eval(observation)
        .unwrap_or_else(|error| panic!("Rust rejected lexical-for observation: {error}"));
    assert_eq!(
        normalize_rust_value(value),
        oracle_two_eval_value_observation(&oracle, setup, observation),
    );
}

fn oracle_two_eval_value_observation(oracle: &OsStr, setup: &str, observation: &str) -> String {
    let script = "std.evalScript(scriptArgs[0]);var __qjo_value=std.evalScript(scriptArgs[1]);print(typeof __qjo_value+'|'+String(__qjo_value));";
    let output = Command::new(oracle)
        .args(["--std", "-e", script, setup, observation])
        .output()
        .unwrap_or_else(|error| panic!("could not run QuickJS cross-eval probe: {error}"));
    assert!(
        output.status.success(),
        "pinned QuickJS rejected cross-eval probe: {}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout)
        .expect("QuickJS cross-eval output was not UTF-8")
        .trim_end()
        .to_owned()
}
