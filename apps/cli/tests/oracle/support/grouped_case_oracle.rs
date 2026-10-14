// Shared drivers for `GroupedCase` tables. Each topic module keeps its own
// cases, prelude and label; these helpers only own the comparison loops.
use crate::oracle_cases::GroupedCase as Case;
use crate::runtime_observation::{host_runtime_context, primitive_value_text, string_property};
use crate::runtime_oracle::value_type;
use quickjs_oxide::engine::api::{Context, Runtime, RuntimeError, Value};
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

/// Assert that pinned QuickJS still produces each frozen prelude-case vector.
pub(crate) fn self_check_prelude_cases(label: &str, prelude: &str, cases: &[Case]) {
    let oracle = require_pinned_qjs_oracle!("{label} oracle self-check");
    let mut failures = Vec::new();
    for case in cases {
        let actual = prelude_oracle_observation(&oracle, prelude, case);
        if actual != case.expected {
            failures.push(format!(
                "{} / {}\nactual: {:?}\nexpected: {:?}",
                case.group, case.description, actual, case.expected,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "pinned QuickJS {label} vectors drifted in {} case(s):\n\n{}",
        failures.len(),
        failures.join("\n\n"),
    );
}

/// Compare the selected prelude-case groups between Rust and pinned QuickJS.
pub(crate) fn compare_prelude_case_groups(
    label: &str,
    prelude: &str,
    cases: &[Case],
    groups: &[&str],
) {
    let oracle = require_pinned_qjs_oracle!("{label} differential");
    let mut failures = Vec::new();
    for case in cases.iter().filter(|case| groups.contains(&case.group)) {
        let (runtime, mut context) = host_runtime_context();
        let actual = prelude_rust_observation(&runtime, &mut context, prelude, case);
        let expected = prelude_oracle_observation(&oracle, prelude, case);
        if actual != expected {
            failures.push(format!(
                "{} / {}\nsource: {:?}\noxide: {:?}\noracle: {:?}",
                case.group, case.description, case.source, actual, expected,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{label} semantics drifted in {} case(s):\n\n{}",
        failures.len(),
        failures.join("\n\n"),
    );
}

fn prelude_rust_observation(
    runtime: &Runtime,
    context: &mut Context,
    prelude: &str,
    case: &Case,
) -> String {
    let source = format!("{prelude}\n{}", case.source);
    match context.eval(&source) {
        Ok(value) => format!(
            "return|{}|{}",
            value_type(runtime, &value),
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
                    string_property(runtime, context, &error, "name"),
                    string_property(runtime, context, &error, "message"),
                ),
                value => format!(
                    "throw|{}|{}",
                    value_type(runtime, &value),
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

fn prelude_oracle_observation(oracle: &OsStr, prelude: &str, case: &Case) -> String {
    let source = format!("{prelude}\n{}", case.source);
    let description = format!("{} / {}", case.group, case.description);
    crate::quickjs_oracle::observe_completion(oracle, &source, &description)
}

/// Compare `actual(case)` with `expected(case)` for every case and report all
/// drifts together. `labels` name the two observations in each failure.
pub(crate) fn assert_grouped_cases_agree<'a>(
    cases: impl IntoIterator<Item = &'a Case>,
    summary: &str,
    labels: [&str; 2],
    actual: impl Fn(&Case) -> String,
    expected: impl Fn(&Case) -> String,
) {
    let [actual_label, expected_label] = labels;
    let mut failures = Vec::new();
    for case in cases {
        let actual = actual(case);
        let expected = expected(case);
        if actual != expected {
            failures.push(format!(
                "{} / {}\nsource: {:?}\n{actual_label}: {actual:?}\n{expected_label}: {expected:?}",
                case.group, case.description, case.source,
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{summary} in {} case(s):\n\n{}",
        failures.len(),
        failures.join("\n\n"),
    );
}
