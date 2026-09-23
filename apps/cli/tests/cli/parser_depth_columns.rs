//! B8-r4 B-D: uncaught parser stack-overflow diagnostics must point at the
//! first body/operand token, byte-for-byte like pinned QuickJS 2026-06-04.
//!
//! Before the repair the logical budget fired at the token that *opens* the
//! production, so block/if/label/new/import reported the column one head-width
//! early. The oxide side asserts the exact stderr (message, location, exit
//! code) unconditionally; when `QJS_ORACLE` points at the pinned `qjs`, the
//! same program is run there and the two results must be identical.

use std::process::Command;

fn run(binary: &str, options: &[&str], source: &str) -> (i32, Vec<u8>, Vec<u8>) {
    let output = Command::new(binary)
        .args(options)
        .args(["-e", source])
        .output()
        .expect("run qjs for parser-depth column case");
    (
        output.status.code().unwrap_or(-1),
        output.stdout,
        output.stderr,
    )
}

/// One uncaught overflow case: CLI options, source, and the exact pinned stderr.
struct Case {
    name: &'static str,
    source: String,
    expected_stderr: &'static str,
}

fn repeat(left: &str, leaf: &str, right: &str, depth: usize) -> String {
    format!("{}{leaf}{}", left.repeat(depth), right.repeat(depth))
}

fn cases() -> Vec<Case> {
    vec![
        // Direct roots report `<cmdline>` locations.
        Case {
            name: "direct-block-3270",
            source: repeat("{", "0", "}", 3270),
            expected_stderr: "SyntaxError: stack overflow\n    at <cmdline>:1:3271\n",
        },
        Case {
            name: "direct-if-3442",
            source: repeat("if(1)", "0", "", 3442),
            expected_stderr: "SyntaxError: stack overflow\n    at <cmdline>:1:17211\n",
        },
        Case {
            name: "direct-label-3442",
            source: (0..3442).map(|i| format!("L{i}:")).collect::<String>() + "0",
            expected_stderr: "SyntaxError: stack overflow\n    at <cmdline>:1:19543\n",
        },
        // eval-wrapped sources report the synthesized `<input>` location plus
        // the `<eval>` caller frame, exactly like pinned.
        Case {
            name: "eval-new-noargs-5935",
            source: format!(
                "eval(\"function z(){{return {}}}\")",
                repeat("new ", "Object", "", 5935)
            ),
            expected_stderr: "SyntaxError: stack overflow\n    at <input>:1:23761\n    at <eval> (<cmdline>:1:5)\n",
        },
        Case {
            name: "eval-import-742",
            source: format!(
                "eval(\"function z(){{return {}}}\")",
                repeat("import(", "0", ")", 742)
            ),
            expected_stderr: "SyntaxError: stack overflow\n    at <input>:1:5215\n    at <eval> (<cmdline>:1:5)\n",
        },
    ]
}

#[test]
fn parser_stack_overflow_columns_are_byte_exact() {
    let oxide = env!("CARGO_BIN_EXE_qjs");
    for case in cases() {
        let (code, stdout, stderr) = run(oxide, &[], &case.source);
        assert_eq!(code, 1, "{} must exit 1", case.name);
        assert!(stdout.is_empty(), "{} must print nothing", case.name);
        assert_eq!(
            String::from_utf8(stderr).expect("utf8 stderr"),
            case.expected_stderr,
            "{} oxide stderr location drifted",
            case.name,
        );
    }
}

#[test]
fn parser_stack_overflow_columns_match_pinned_oracle() {
    let Some(oracle) = std::env::var_os("QJS_ORACLE") else {
        eprintln!("SKIP parser-depth column differential: set QJS_ORACLE to the pinned qjs");
        return;
    };
    let oracle = oracle.to_string_lossy().into_owned();
    let oxide = env!("CARGO_BIN_EXE_qjs").to_owned();
    for case in cases() {
        let oxide_result = run(&oxide, &[], &case.source);
        let pinned = run(&oracle, &[], &case.source);
        let oxide_triple = (
            oxide_result.0,
            oxide_result.1.clone(),
            oxide_result.2.clone(),
        );
        assert_eq!(
            oxide_triple, pinned,
            "{} differs from pinned QuickJS (code/stdout/stderr)",
            case.name
        );
    }
}
