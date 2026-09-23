//! B8-r5 N1 regression: the immediate array operand of a spread element must
//! still parse the *complete* AssignmentExpression. A repair in B8-r4 charged
//! the leading `[` by calling `parse_array_literal` directly, which truncated
//! the expression right after the literal and rejected shallow programs that
//! pinned QuickJS 2026-06-04 accepts (e.g. `[...[1].map(x=>x)]`).
//!
//! Every case here is additionally compared byte-for-byte against
//! `$QJS_ORACLE` when that variable points at the pinned `qjs` binary.

use quickjs_oxide::engine::api::{Runtime, Value};
use quickjs_oxide_host::SystemHostServices;

mod common;

/// A spread-operand case: `source` must complete (JSON of the outer array in
/// `expected`) or throw the exact `name:message` label.
struct SpreadCase {
    /// The expression after `...`, without the surrounding array.
    operand: &'static str,
    /// JSON text when the program completes, or the thrown label.
    expected: &'static str,
    throws: bool,
}

const VALUE_CASES: &[SpreadCase] = &[
    // Postfix member + call continuation.
    SpreadCase {
        operand: "[1].map(x=>x)",
        expected: "[1]",
        throws: false,
    },
    SpreadCase {
        operand: "[1].concat([2])",
        expected: "[1,2]",
        throws: false,
    },
    // Member-index continuation followed by a conditional.
    SpreadCase {
        operand: "[1][0] ? [7] : [8]",
        expected: "[7]",
        throws: false,
    },
    // Logical continuation.
    SpreadCase {
        operand: "[1] || []",
        expected: "[1]",
        throws: false,
    },
    // Binary continuation: "1,2" spread into characters.
    SpreadCase {
        operand: "[1,2] + ''",
        expected: "[\"1\",\",\",\"2\"]",
        throws: false,
    },
    // Exponentiation feeding a conditional continuation.
    SpreadCase {
        operand: "[2] ** 0 ? [9] : [8]",
        expected: "[9]",
        throws: false,
    },
    // Conditional continuation.
    SpreadCase {
        operand: "[1] ? [2] : [3]",
        expected: "[2]",
        throws: false,
    },
    // Parenthesized control: the ordinary non-immediate path.
    SpreadCase {
        operand: "([1])",
        expected: "[1]",
        throws: false,
    },
    // Plain immediate array literal (spread-sngl-literal).
    SpreadCase {
        operand: "[3,4,5]",
        expected: "[3,4,5]",
        throws: false,
    },
    // Assignment continuation: the value of `a=[7,8]` is the RHS array.
    SpreadCase {
        operand: "a=[7,8]",
        expected: "[7,8]",
        throws: false,
    },
    // An array assignment pattern is a legal spread operand.
    SpreadCase {
        operand: "[x]=[5]",
        expected: "[5]",
        throws: false,
    },
];

const THROW_CASES: &[SpreadCase] = &[
    // Member-index continuation: parse succeeds, spreading 1 throws at run
    // time (it must not be a parse-time rejection).
    SpreadCase {
        operand: "[1][0]",
        expected: "TypeError:value is not iterable",
        throws: true,
    },
];

fn eval_json(operand: &str) -> String {
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    let mut context = runtime.new_context();
    match context.eval(&format!("JSON.stringify([...{operand}])")) {
        Ok(Value::String(value)) => value.to_utf8_lossy(),
        Ok(other) => panic!("expected a string completion, got {other:?} for {operand}"),
        Err(error) => panic!("evaluation failed: {error} for {operand}"),
    }
}

fn eval_throw_label(operand: &str) -> String {
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    let mut context = runtime.new_context();
    let program = format!("try{{[...{operand}];\"OK\"}}catch(e){{e.name+\":\"+e.message}}");
    match context.eval(&program) {
        Ok(Value::String(value)) => value.to_utf8_lossy(),
        Ok(other) => panic!("expected a label string, got {other:?} for {operand}"),
        Err(error) => panic!("label probe failed: {error} for {operand}"),
    }
}

#[test]
fn spread_operands_accept_full_assignment_expressions() {
    for case in VALUE_CASES {
        assert_eq!(
            eval_json(case.operand),
            case.expected,
            "spread operand {:?} regressed",
            case.operand
        );
    }
}

#[test]
fn spread_member_index_operand_fails_at_runtime_not_parse_time() {
    for case in THROW_CASES {
        assert_eq!(
            eval_throw_label(case.operand),
            case.expected,
            "spread operand {:?} must throw the pinned runtime error",
            case.operand
        );
    }
}

#[test]
fn spread_arrow_head_operand_is_still_a_syntax_error() {
    // Pinned QuickJS: `[...[x]=>x]` -> SyntaxError: expecting ']' at 1:8.
    // The leading bracket is an arrow head there, never an array primary.
    assert_eq!(common::compile_syntax_error("[...[x]=>x]"), "expecting ']'");
}

/// Run the complete `-e` program (as the qjs CLI sees it) and return
/// `(rc, stdout, stderr)`.
fn run_qjs(binary: &str, program: &str) -> (i32, String, String) {
    let output = std::process::Command::new(binary)
        .arg("-e")
        .arg(program)
        .output()
        .unwrap_or_else(|error| panic!("run {binary}: {error}"));
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8(output.stdout).expect("qjs utf8 stdout"),
        String::from_utf8(output.stderr).expect("qjs utf8 stderr"),
    )
}

#[test]
fn spread_operands_match_pinned_qjs_byte_for_byte() {
    let Ok(oracle) = std::env::var("QJS_ORACLE") else {
        eprintln!("QJS_ORACLE not set; skipping pinned byte comparison");
        return;
    };
    assert!(
        std::path::Path::new(&oracle).exists(),
        "QJS_ORACLE does not exist: {oracle}"
    );

    for case in VALUE_CASES.iter().chain(THROW_CASES) {
        if case.throws {
            let program = format!(
                "try{{[...{op}];print(\"OK\")}}catch(e){{print(e.name+\":\"+e.message)}}",
                op = case.operand
            );
            let (rc, stdout, stderr) = run_qjs(&oracle, &program);
            assert_eq!(rc, 0, "oracle label probe must exit 0: {stderr}");
            assert_eq!(
                stdout.trim_end(),
                case.expected,
                "pinned oracle differs for spread operand {:?}",
                case.operand
            );
        } else {
            let program = format!("print(JSON.stringify([...{}]))", case.operand);
            let (rc, stdout, stderr) = run_qjs(&oracle, &program);
            assert_eq!(rc, 0, "oracle must accept {:?}: {stderr}", case.operand);
            assert_eq!(
                stdout.trim_end(),
                case.expected,
                "pinned oracle differs for spread operand {:?}",
                case.operand
            );
        }
    }

    // The arrow-head rejection, including its 1:8 diagnostic location.
    let (rc, _stdout, stderr) = run_qjs(&oracle, "[...[x]=>x]");
    assert_eq!(rc, 1);
    let stderr = stderr.replace("\r\n", "\n");
    assert!(
        stderr.contains("SyntaxError: expecting ']'") && stderr.contains(":1:8"),
        "pinned diagnostic changed for arrow spread operand: {stderr}"
    );
}
