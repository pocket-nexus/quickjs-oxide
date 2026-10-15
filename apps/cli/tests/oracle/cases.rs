// One test per JavaScript fixture directory under `cases/`; the format and
// comparison modes are documented in `support/js_cases.rs`.
use crate::support::js_cases::{case_directories, run_case_directory};

const DIRECTORIES: &[&str] = &[
    "array",
    "function_semantics",
    "number",
    "object",
    "primitive_intrinsics",
    "regexp",
    "string",
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
fn function_semantics() {
    run_case_directory("function_semantics");
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
fn primitive_intrinsics() {
    run_case_directory("primitive_intrinsics");
}

#[test]
fn regexp() {
    run_case_directory("regexp");
}

#[test]
fn string() {
    run_case_directory("string");
}
