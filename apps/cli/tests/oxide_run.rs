//! The new core's driver: output, exit status and messages
//! (`docs/performance/new-core-phase2.md`, section 5).

use std::path::PathBuf;
use std::process::{Command, Output};

fn script(name: &str, source: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("oxide-run-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(name);
    std::fs::write(&path, source).unwrap();
    path
}

fn oxide_run(options: &[&str], name: &str, source: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_oxide-run"))
        .args(options)
        .arg(script(name, source))
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}

#[test]
fn prints_and_exits_zero() {
    let output = oxide_run(
        &[],
        "ok.js",
        "function f(n) { return n * 2; } print(f(21), 'x');",
    );
    assert!(output.status.success());
    assert_eq!(text(&output.stdout), "42 x\n");
}

#[test]
fn an_uncaught_exception_exits_one_with_its_string() {
    let output = oxide_run(&[], "throw.js", "print('before'); null.x;");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stdout), "before\n");
    assert_eq!(
        text(&output.stderr),
        "TypeError: cannot read property 'x' of null\n"
    );
}

#[test]
fn a_dropped_feature_is_an_early_syntax_error() {
    let output = oxide_run(&[], "with.js", "print('before'); with ({}) {}");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stdout), "");
    assert_eq!(
        text(&output.stderr),
        "SyntaxError: with statements are not supported\n"
    );
}

#[test]
fn disassemble_prints_every_unit() {
    let output = oxide_run(
        &["--disassemble"],
        "dis.js",
        "function f(a) { return a + 1; }",
    );
    assert!(output.status.success());
    let listing = text(&output.stdout);
    assert!(listing.contains("f: params 1"), "{listing}");
    assert!(listing.contains("AddI"), "{listing}");
}

#[test]
fn the_heap_cap_turns_into_an_out_of_memory_error() {
    let output = oxide_run(
        &["--heap-cap-mib", "8"],
        "cap.js",
        "var keep = []; for (var i = 0; ; i++) keep[i] = { i: i };",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stderr), "InternalError: out of memory\n");
}

#[test]
fn gc_stress_needs_the_feature() {
    let output = oxide_run(&["--gc-stress=1"], "stress.js", "print(1)");
    if cfg!(feature = "new-core-gc-stress") {
        assert!(output.status.success());
        assert_eq!(text(&output.stdout), "1\n");
    } else {
        assert_eq!(output.status.code(), Some(1));
        assert!(text(&output.stderr).contains("new-core-gc-stress"));
    }
}
