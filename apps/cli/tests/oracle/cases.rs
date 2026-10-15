// One test per JavaScript fixture directory under `cases/`; the format and
// comparison modes are documented in `support/js_cases.rs`.
use crate::support::js_cases::{case_directories, run_case_directory};

const DIRECTORIES: &[&str] = &["number", "primitive_intrinsics"];

#[test]
fn every_case_directory_has_a_test() {
    assert_eq!(case_directories(), DIRECTORIES);
}

#[test]
fn number() {
    run_case_directory("number");
}

#[test]
fn primitive_intrinsics() {
    run_case_directory("primitive_intrinsics");
}
