// Shared loops for `ExpectedCase` tables whose scripts return one string.
// Each topic keeps its own cases and optional source prelude.
use crate::oracle_cases::ExpectedCase as Case;
use crate::runtime_observation::host_runtime_context;
use crate::support::quickjs_typed_array_oracle::observe_string_value;
use quickjs_oxide::engine::api::{RuntimeError, Value};
use std::ffi::OsStr;

fn case_source(prelude: Option<&str>, case: &Case) -> String {
    match prelude {
        Some(prelude) => format!("{prelude}\n{}", case.source),
        None => case.source.to_owned(),
    }
}

/// Evaluate a case in a fresh host runtime; it must return a string.
pub(crate) fn oxide_string_observation(prelude: Option<&str>, case: &Case) -> String {
    let (_runtime, mut context) = host_runtime_context();
    match context.eval(&case_source(prelude, case)) {
        Ok(Value::String(value)) => value.to_utf8_lossy(),
        Ok(value) => panic!(
            "Oxide returned a non-string for {}: {value:?}",
            case.description,
        ),
        Err(RuntimeError::Exception) => {
            let exception = context.take_exception().unwrap();
            panic!("Oxide threw for {}: {exception:?}", case.description);
        }
        Err(error) => panic!("Oxide failed for {}: {error}", case.description),
    }
}

/// Run a case through pinned QuickJS and print its string value.
pub(crate) fn oracle_string_observation(
    oracle: &OsStr,
    prelude: Option<&str>,
    case: &Case,
) -> String {
    observe_string_value(oracle, &case_source(prelude, case), case.description)
}

pub(crate) fn assert_string_cases_match_frozen(prelude: Option<&str>, cases: &[Case]) {
    for case in cases {
        assert_eq!(
            oxide_string_observation(prelude, case),
            case.expected,
            "{}",
            case.description,
        );
    }
}

pub(crate) fn assert_quickjs_string_cases_match_frozen(
    oracle: &OsStr,
    prelude: Option<&str>,
    cases: &[Case],
) {
    for case in cases {
        assert_eq!(
            oracle_string_observation(oracle, prelude, case),
            case.expected,
            "{}",
            case.description,
        );
    }
}

pub(crate) fn assert_string_cases_match_quickjs(
    oracle: &OsStr,
    prelude: Option<&str>,
    cases: &[Case],
) {
    for case in cases {
        assert_eq!(
            oxide_string_observation(prelude, case),
            oracle_string_observation(oracle, prelude, case),
            "{}",
            case.description,
        );
    }
}
