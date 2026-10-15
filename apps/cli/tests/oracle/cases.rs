// One test per JavaScript fixture directory under `cases/`; the format and
// comparison modes are documented in `support/js_cases.rs`.
use crate::support::js_cases::{case_directories, run_case_directory};

const DIRECTORIES: &[&str] = &[
    "array",
    "async_functions",
    "async_methods",
    "binary_data",
    "collections",
    "control_flow",
    "errors",
    "eval",
    "function_semantics",
    "global",
    "json",
    "member_access",
    "number",
    "object",
    "operators",
    "primitive_intrinsics",
    "program_declarations",
    "proxy_reflect",
    "regexp",
    "string",
    "typed_array",
];

#[test]
fn every_case_directory_has_a_test() {
    assert_eq!(case_directories(), DIRECTORIES);
}

#[test]
fn array() {
    run_case_directory("array");
}

#[test]
fn async_functions() {
    run_case_directory("async_functions");
}

#[test]
fn async_methods() {
    run_case_directory("async_methods");
}

#[test]
fn binary_data() {
    run_case_directory("binary_data");
}

#[test]
fn collections() {
    run_case_directory("collections");
}

#[test]
fn control_flow() {
    run_case_directory("control_flow");
}

#[test]
fn errors() {
    run_case_directory("errors");
}

#[test]
fn eval() {
    run_case_directory("eval");
}

#[test]
fn function_semantics() {
    run_case_directory("function_semantics");
}

#[test]
fn global() {
    run_case_directory("global");
}

#[test]
fn json() {
    run_case_directory("json");
}

#[test]
fn member_access() {
    run_case_directory("member_access");
}

#[test]
fn number() {
    run_case_directory("number");
}

#[test]
fn object() {
    run_case_directory("object");
}

#[test]
fn operators() {
    run_case_directory("operators");
}

#[test]
fn primitive_intrinsics() {
    run_case_directory("primitive_intrinsics");
}

#[test]
fn program_declarations() {
    run_case_directory("program_declarations");
}

#[test]
fn proxy_reflect() {
    run_case_directory("proxy_reflect");
}

#[test]
fn regexp() {
    run_case_directory("regexp");
}

#[test]
fn string() {
    run_case_directory("string");
}

#[test]
fn typed_array() {
    run_case_directory("typed_array");
}
