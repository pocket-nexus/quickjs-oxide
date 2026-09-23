//! Deterministic main-thread parser stack-guard probe (B8-r4, B8-r5 N2).
//!
//! libtest always runs `#[test]` bodies on runtime worker threads, whose
//! stacks are fixed-size non-growable mappings. The parser guard's
//! growable-main-stack branch (and its `RLIMIT_STACK=unlimited` handling) can
//! only be exercised on the real main thread of a standalone process.
//!
//! This is a `harness = false` integration test (see the `[[test]]` entry in
//! the workspace-root `Cargo.toml`). Cargo builds every test target before
//! running any test binary, so the probe is *this* executable and can never
//! be missing in a clean `cargo test --workspace --all-targets` run; the old
//! arrangement located a separately built example binary by hand and failed
//! from a fresh target directory unless that example had been built first.
//!
//! Modes:
//! * default (no marker) — run the full 18-case matrix: RLIMIT_STACK 4096 and
//!   8192 KiB plus `unlimited`, crossed with native recursion depths 0, 1, 2,
//!   4, 8 and 16 (~64 KiB per frame). Every row must exit 0, print exactly
//!   `SyntaxError:stack overflow` on stdout, and emit no stderr. This is what
//!   `cargo test` executes.
//! * child (env `QJS_OXIDE_MAIN_THREAD_STACK_PROBE_CHILD=1`) — run one probe
//!   on the main thread:
//!   - `native F N` — warm up with a shallow eval near the top of the main
//!     stack, recurse `F` native frames, then parse
//!     `class C{static{`×N `0` `}}`×N (a production with no per-level logical
//!     guard charge). Prints `THROW:<name>:<message>` and exits 0 when the
//!     guard surfaces the catchable overflow; an unwarned host overflow
//!     aborts.
//!   - `shallow N` — evaluate `(`×N `1` `)`×N and print
//!     `ACCEPTED` / `THROW:<name>:<message>` (exit 0 either way).
//!
//! Manual invocation:
//!
//! ```sh
//! QJS_OXIDE_MAIN_THREAD_STACK_PROBE_CHILD=1 \
//!   target/debug/deps/main_thread_stack_probe-<hash> native 1 6000
//! ```

use quickjs_oxide::engine::api::{Runtime, RuntimeError, Value};
use quickjs_oxide_host::SystemHostServices;
use std::hint::black_box;
use std::process::{Command, ExitCode, ExitStatus};
use std::time::{Duration, Instant};

const CHILD_ENV: &str = "QJS_OXIDE_MAIN_THREAD_STACK_PROBE_CHILD";
const PARSE_DEPTH: usize = 6000;
const STACK_LIMITS: &[&str] = &["4096", "8192", "unlimited"];
const NATIVE_FRAMES: &[&str] = &["0", "1", "2", "4", "8", "16"];
const ROW_TIMEOUT: Duration = Duration::from_secs(60);

#[inline(never)]
fn descend(depth: usize, run: &mut dyn FnMut() -> String) -> String {
    let mut pad = [0u8; 64 * 1024];
    pad[depth % pad.len()] = depth as u8;
    black_box(&mut pad);
    if depth == 0 {
        return run();
    }
    let result = descend(depth - 1, run);
    black_box(pad[0]);
    result
}

fn label_of(value: Result<Value, RuntimeError>) -> String {
    match value {
        Ok(Value::String(label)) => label.to_utf8_lossy(),
        Ok(other) => format!("unexpected:{other:?}"),
        Err(RuntimeError::Exception) => "UNCAUGHT".to_owned(),
        Err(error) => format!("ERROR:{error}"),
    }
}

/// Run one probe on this process's real main thread. Returns the process exit
/// code; the outcome line is printed to stdout.
fn run_child(mut arguments: impl Iterator<Item = String>) -> ExitCode {
    let mode = arguments.next().unwrap_or_else(|| "native".to_owned());
    let first: usize = arguments
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let second: usize = arguments
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(PARSE_DEPTH);

    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    let mut context = runtime.new_context();

    match mode.as_str() {
        "shallow" => {
            let source = format!("({}1{})", "(".repeat(first), ")".repeat(first));
            let program =
                format!("try{{eval({source:?});\"ACCEPTED\"}}catch(e){{e.name+\":\"+e.message}}");
            println!("{}", label_of(context.eval(&program)));
        }
        "native" => {
            // The first parser on the thread populates the per-thread stack
            // metadata cache near the top, exactly as a host embedding which
            // constructs its runtime before recursing natively would.
            match context.eval("1+1") {
                Ok(Value::Int(2)) => {}
                other => {
                    eprintln!("warm-up eval failed: {other:?}");
                    return ExitCode::from(2);
                }
            }
            // `class C{static{` is a production whose nested levels carry no
            // per-level logical guard charge, so only the physical token-edge
            // backstop can stop the parse before the host stack overflows.
            let nested_source = format!(
                "{}0{}",
                "class C{static{".repeat(second),
                "}}".repeat(second)
            );
            let mut encoded = String::with_capacity(nested_source.len() + 2);
            encoded.push('"');
            for ch in nested_source.chars() {
                match ch {
                    '"' => encoded.push_str("\\\""),
                    '\\' => encoded.push_str("\\\\"),
                    '\n' => encoded.push_str("\\n"),
                    '\r' => encoded.push_str("\\r"),
                    _ => encoded.push(ch),
                }
            }
            encoded.push('"');
            let program = format!(
                "try{{eval({encoded});\"UNEXPECTED:ACCEPTED\"}}catch(e){{e.name+\":\"+e.message}}"
            );
            let mut run = || label_of(context.eval(&program));
            println!("{}", descend(first, &mut run));
        }
        other => {
            eprintln!("unknown mode {other}; expected `shallow` or `native`");
            return ExitCode::from(2);
        }
    }
    ExitCode::SUCCESS
}

/// One matrix row: spawn the probe under `ulimit -s <limit>`, at native frame
/// depth `frames`. `set -e`-style chaining means a refused `ulimit` fails the
/// row instead of masking a sandbox refusal.
struct RowResult {
    status: Option<ExitStatus>,
    stdout: String,
    stderr: String,
}

fn run_row(probe: &str, limit: &'static str, frames: &'static str) -> RowResult {
    let mut child = Command::new("bash")
        .args([
            "-c",
            "ulimit -s \"$1\" && shift && exec \"$@\"",
            "main-thread-stack-probe",
            limit,
            probe,
            "native",
            frames,
            &PARSE_DEPTH.to_string(),
        ])
        .env(CHILD_ENV, "1")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn main-thread stack probe");

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if started.elapsed() >= ROW_TIMEOUT {
                    let _ = child.kill();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(error) => panic!("wait for main-thread stack probe: {error}"),
        }
    };
    let output = child
        .wait_with_output()
        .expect("collect probe output after kill");
    RowResult {
        status,
        stdout: String::from_utf8(output.stdout).expect("probe utf8 stdout"),
        stderr: String::from_utf8(output.stderr).expect("probe utf8 stderr"),
    }
}

/// Matrix entry point (the deterministic test body).
fn run_matrix() -> ExitCode {
    let probe = std::env::current_exe().expect("locate this probe test binary");
    let probe = probe.to_str().expect("probe path is utf-8");

    let mut rows = 0usize;
    let mut failures = Vec::new();
    for limit in STACK_LIMITS {
        for frames in NATIVE_FRAMES {
            rows += 1;
            let result = run_row(probe, limit, frames);
            let ok = match &result.status {
                Some(status) if status.success() => {
                    result.stdout.trim() == "SyntaxError:stack overflow" && result.stderr.is_empty()
                }
                Some(status) => {
                    eprintln!(
                        "limit={limit} frames={frames} exited {status}: {}",
                        result.stderr.trim()
                    );
                    false
                }
                None => {
                    eprintln!(
                        "limit={limit} frames={frames} timed out after {ROW_TIMEOUT:?} (aborted?)"
                    );
                    false
                }
            };
            if ok {
                println!("ok  limit={limit:<9} frames={frames:<2} -> catchable SyntaxError");
            } else {
                eprintln!(
                    "BAD limit={limit} frames={frames} stdout={:?} stderr={:?}",
                    result.stdout, result.stderr
                );
                failures.push((limit, frames));
            }
        }
    }

    if failures.is_empty() {
        println!("main-thread stack probe: {rows}/{rows} catchable, 0 abort/timeout");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "main-thread stack probe: {} failed rows: {failures:?}",
            failures.len()
        );
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    if std::env::var_os(CHILD_ENV).is_some() {
        return run_child(std::env::args().skip(1));
    }
    run_matrix()
}
