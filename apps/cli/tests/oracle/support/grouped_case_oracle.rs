// Shared drivers for `GroupedCase` tables. Each topic module keeps its own
// cases, prelude and label; these helpers only own the comparison loops.
use crate::oracle_cases::GroupedCase as Case;
use crate::runtime_observation::{host_runtime_context, primitive_value_text};
use std::ffi::OsStr;

/// Assert every case against its frozen expectation using a Rust observer.
pub(crate) fn assert_cases_match_expectations(
    label: &str,
    cases: &[Case],
    rust_observation: impl Fn(&Case) -> String,
) {
    for case in cases {
        assert_eq!(
            rust_observation(case),
            case.expected,
            "Rust {label} behavior drifted for {} / {}: {:?}",
            case.group,
            case.description,
            case.source,
        );
    }
}

/// Check every pinned QuickJS observation first, then compare Rust against it.
pub(crate) fn assert_cases_match_quickjs(
    label: &str,
    skip: &str,
    cases: &[Case],
    oracle_observation: impl Fn(&OsStr, &Case) -> String,
    rust_observation: impl Fn(&Case) -> String,
) {
    let oracle = require_pinned_qjs_oracle!("{skip}");

    // Check every pinned QuickJS result before entering the Rust comparison so
    // an oracle drift is distinguishable from a parser/runtime gap.
    let quickjs = cases
        .iter()
        .map(|case| oracle_observation(&oracle, case))
        .collect::<Vec<_>>();
    for (case, observation) in cases.iter().zip(&quickjs) {
        assert_eq!(
            observation, case.expected,
            "pinned QuickJS {label} vector drifted for {} / {}: {:?}",
            case.group, case.description, case.source,
        );
    }
    for (case, observation) in cases.iter().zip(&quickjs) {
        assert_eq!(
            rust_observation(case),
            *observation,
            "{label} behavior differed from pinned QuickJS for {} / {}: {:?}",
            case.group,
            case.description,
            case.source,
        );
    }
}

/// Evaluate a case in a fresh host runtime; the case must return normally.
pub(crate) fn rust_return_observation(label: &str, case: &Case) -> String {
    let (runtime, mut context) = host_runtime_context();
    let value = context.eval(case.source).unwrap_or_else(|error| {
        panic!(
            "Rust rejected {label} probe {} / {} ({:?}): {error}",
            case.group, case.description, case.source,
        )
    });
    format!(
        "return|{}|{}",
        crate::runtime_observation::checked_value_type(&runtime, &value),
        primitive_value_text(value),
    )
}
