// One test per JavaScript fixture directory under `cases/`; the format and
// comparison modes are documented in `support/js_cases.rs`.
use crate::support::js_cases::{case_directories, run_case_directory};

const DIRECTORIES: &[&str] = &[
    "arguments",
    "array",
    "async_functions",
    "async_methods",
    "atomics_non_shared",
    "binary_data",
    "class_initialization",
    "collections",
    "control_flow",
    "date",
    "errors",
    "eval",
    "for_await_of",
    "function_declarations",
    "function_semantics",
    "global",
    "iterator",
    "json",
    "lexical",
    "math",
    "member_access",
    "number",
    "object",
    "operators",
    "parameters",
    "primitive_intrinsics",
    "primitives",
    "program_declarations",
    "proxy_reflect",
    "regexp",
    "string",
    "typed_array",
    "unicode_lexical",
    "update",
    "vm_object_coercion",
];

#[test]
fn every_case_directory_has_a_test() {
    assert_eq!(case_directories(), DIRECTORIES);
}

#[test]
fn arguments() {
    run_case_directory("arguments");
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
fn atomics_non_shared() {
    run_case_directory("atomics_non_shared");
}

#[test]
fn binary_data() {
    run_case_directory("binary_data");
}

#[test]
fn class_initialization() {
    run_case_directory("class_initialization");
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
fn date() {
    run_case_directory("date");
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
fn for_await_of() {
    run_case_directory("for_await_of");
}

#[test]
fn function_declarations() {
    run_case_directory("function_declarations");
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
fn iterator() {
    run_case_directory("iterator");
}

#[test]
fn json() {
    run_case_directory("json");
}

#[test]
fn lexical() {
    run_case_directory("lexical");
}

#[test]
fn math() {
    run_case_directory("math");
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
fn parameters() {
    run_case_directory("parameters");
}

#[test]
fn primitive_intrinsics() {
    run_case_directory("primitive_intrinsics");
}

#[test]
fn primitives() {
    run_case_directory("primitives");
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

#[test]
fn unicode_lexical() {
    run_case_directory("unicode_lexical");
}

#[test]
fn update() {
    run_case_directory("update");
}

#[test]
fn vm_object_coercion() {
    run_case_directory("vm_object_coercion");
}
