//! The core's options of `qjs` beyond QuickJS's: disassembly, the heap cap,
//! the stress collector and the reports.

use std::path::PathBuf;
use std::process::{Command, Output};

fn script(name: &str, source: &str) -> PathBuf {
    let directory =
        std::env::temp_dir().join(format!("qjs-core-options-test-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join(name);
    std::fs::write(&path, source).unwrap();
    path
}

fn qjs(options: &[&str], name: &str, source: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_qjs"))
        .args(options)
        .arg(script(name, source))
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap()
}

#[test]
fn a_dropped_feature_is_an_early_syntax_error() {
    let output = qjs(&[], "with.js", "print('before'); with ({}) {}");
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stdout), "");
    assert!(
        text(&output.stderr).starts_with("SyntaxError: with statements are not supported\n"),
        "{}",
        text(&output.stderr)
    );
}

#[test]
fn disassemble_prints_every_unit() {
    let output = qjs(
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
    let output = qjs(
        &["--heap-cap-mib", "8"],
        "cap.js",
        "var keep = []; for (var i = 0; ; i++) keep[i] = { i: i };",
    );
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(text(&output.stderr), "InternalError: out of memory\n");
}

#[test]
fn gc_stress_needs_the_feature() {
    let output = qjs(&["--gc-stress=1"], "stress.js", "print(1)");
    if cfg!(feature = "gc-stress") {
        assert!(output.status.success());
        assert_eq!(text(&output.stdout), "1\n");
    } else {
        assert_eq!(output.status.code(), Some(2));
        assert!(text(&output.stderr).contains("gc-stress"));
    }
}

#[test]
fn the_reports_go_to_stderr_at_exit() {
    let output = qjs(
        &[
            "--gc-stats",
            "--memory-report",
            "--min-heap-mib",
            "1",
            "--gc-growth",
            "1.5",
        ],
        "reports.js",
        "var keep = []; for (var i = 0; i < 100000; i++) keep.push({ i }); print(keep.length);",
    );
    assert!(output.status.success(), "{}", text(&output.stderr));
    assert_eq!(text(&output.stdout), "100000\n");
    let stderr = text(&output.stderr);
    assert!(stderr.contains("gc: "), "{stderr}");
    assert!(stderr.contains("objects"), "{stderr}");
    assert!(stderr.contains("heap reserved"), "{stderr}");
}

#[test]
fn ic_stats_needs_the_feature() {
    let output = qjs(&["--ic-stats"], "ic.js", "print(1)");
    if cfg!(feature = "ic-stats") {
        assert!(output.status.success());
        assert!(text(&output.stderr).contains("mono hits"));
    } else {
        assert_eq!(output.status.code(), Some(2));
        assert!(text(&output.stderr).contains("ic-stats"));
    }
}
