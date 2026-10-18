use std::fs;
use std::io::Read;

use std::path::Path;
use std::process::{Command, Stdio};
use std::rc::Rc;
use std::thread;

use std::time::{Duration, Instant};

use super::admissions::AdmissionCatalog;
use super::metadata::{Metadata, parse_metadata};
use super::report::WorkerResult;
use super::requirements::{
    HostCapabilities, exact_module_test, is_exact_agent_host_test,
    is_exact_dynamic_import_script_test,
};
use super::{Variant, WorkerOptions, validate_relative_test_path};

#[path = "worker.rs"]
mod worker;

pub(super) const WORKER_HOST_CAPABILITIES: HostCapabilities = HostCapabilities {
    agent: false,
    can_block_false: true,
    create_realm: true,
    detach_array_buffer: true,
    eval_script: true,
    gc: true,
    global: true,
    is_html_dda: true,
};

const ASYNC_COMPLETE: &str = "Test262:AsyncTestComplete";
const ASYNC_FAILURE_PREFIX: &str = "Test262:AsyncTestFailure:";

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExceptionDiagnostic {
    error_type: String,
    message: String,
    line: Option<u32>,
    column: Option<u32>,
}

pub(super) struct IsolatedWorkerOptions<'a> {
    pub(super) executable: &'a Path,
    pub(super) suite: &'a Path,
    pub(super) test: &'a Path,
    pub(super) admissions: &'a Path,
    pub(super) admissions_sha256: &'a str,
    pub(super) variant: Variant,
    pub(super) timeout: Duration,
    pub(super) allow_async_host: bool,
    pub(super) allow_agent_host: bool,
}

pub(super) fn run_isolated_worker(
    options: IsolatedWorkerOptions<'_>,
) -> Result<WorkerResult, String> {
    let IsolatedWorkerOptions {
        executable,
        suite,
        test,
        admissions,
        admissions_sha256,
        variant,
        timeout,
        allow_async_host,
        allow_agent_host,
    } = options;
    let mut command = Command::new(executable);
    command
        .arg("--worker-one")
        .arg("--suite")
        .arg(suite)
        .arg("--test")
        .arg(test)
        .arg("--admissions")
        .arg(admissions)
        .arg("--admissions-sha256")
        .arg(admissions_sha256)
        .arg("--variant")
        .arg(variant.name());
    if allow_async_host {
        command.arg("--allow-async-host");
    }
    if allow_agent_host {
        command.arg("--allow-agent-host");
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("spawn worker for {}: {error}", test.display()))?;
    let Some(stdout) = child.stdout.take() else {
        terminate_and_reap(&mut child);
        return Err("worker stdout pipe was missing".to_owned());
    };
    let Some(stderr) = child.stderr.take() else {
        terminate_and_reap(&mut child);
        return Err("worker stderr pipe was missing".to_owned());
    };
    let stdout_reader = spawn_pipe_reader(stdout, "stdout");
    let stderr_reader = spawn_pipe_reader(stderr, "stderr");
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Err(error) => {
                terminate_and_reap(&mut child);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Err(format!("wait for {}: {error}", test.display()));
            }
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                terminate_and_reap(&mut child);
                let _ = stdout_reader.join();
                let _ = stderr_reader.join();
                return Ok(WorkerResult::failure(
                    "timeout",
                    "host",
                    "",
                    format!("worker exceeded {} ms", timeout.as_millis()),
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
        }
    };
    let stdout = join_pipe_reader(stdout_reader, "stdout");
    let stderr = join_pipe_reader(stderr_reader, "stderr");
    let stdout = stdout?;
    let stderr = stderr?;
    if !status.success() {
        return Ok(WorkerResult::failure(
            "crash",
            "host",
            "",
            format!("worker exited with {status}: {}", stderr.trim()),
        ));
    }
    WorkerResult::decode(&stdout).map_err(|error| {
        format!(
            "decode worker for {}: {error}; stderr={:?}",
            test.display(),
            stderr.trim()
        )
    })
}

fn terminate_and_reap(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn spawn_pipe_reader(
    mut pipe: impl Read + Send + 'static,
    name: &'static str,
) -> thread::JoinHandle<Result<String, String>> {
    thread::spawn(move || {
        let mut output = String::new();
        pipe.read_to_string(&mut output)
            .map_err(|error| format!("read worker {name}: {error}"))?;
        Ok(output)
    })
}

fn join_pipe_reader(
    reader: thread::JoinHandle<Result<String, String>>,
    name: &str,
) -> Result<String, String> {
    reader
        .join()
        .map_err(|_| format!("worker {name} reader panicked"))?
}

pub(super) fn run_worker(options: &WorkerOptions) -> Result<WorkerResult, String> {
    validate_relative_test_path(&options.test)?;
    let admissions = Rc::new(AdmissionCatalog::load(
        &options.admissions,
        &options.admissions_sha256,
    )?);
    let path = options.suite.join(&options.test);
    let source =
        fs::read_to_string(&path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let metadata = parse_metadata(&source)?;
    let exact_module = exact_module_test(
        &admissions,
        &options.suite,
        &options.test,
        &source,
        &metadata,
    )?;
    let exact_dynamic_import = is_exact_dynamic_import_script_test(
        &admissions,
        &options.suite,
        &options.test,
        &source,
        &metadata,
    )?;
    let dynamic_import_policy = if exact_dynamic_import {
        Some(
            admissions
                .dynamic_import_root(&options.test)
                .and_then(|root| root.dynamic_import_policy)
                .ok_or_else(|| {
                    format!(
                        "authenticated dynamic import root has no root policy: {}",
                        options.test.display()
                    )
                })?,
        )
    } else {
        None
    };
    if options.allow_agent_host
        && !is_exact_agent_host_test(&admissions, &options.test, &source, &metadata)?
    {
        return Err(format!(
            "Test262 agent host worker rejected unaudited path: {}",
            options.test.display()
        ));
    }
    if (metadata.is_module() && exact_module.is_none())
        || (metadata.is_async() && !options.allow_async_host)
    {
        return Err("unsupported test reached worker".to_owned());
    }
    worker::run_worker(worker::WorkerTest {
        options,
        admissions,
        path: &path,
        source,
        metadata: &metadata,
        exact_module,
        exact_dynamic_import,
        dynamic_import_policy,
    })
}

fn classify_async_print_log(metadata: &Metadata, messages: &[String]) -> WorkerResult {
    let reports = messages
        .iter()
        .filter(|message| {
            message.as_str() == ASYNC_COMPLETE || message.starts_with(ASYNC_FAILURE_PREFIX)
        })
        .collect::<Vec<_>>();
    if reports.len() != 1 {
        return WorkerResult::failure(
            "fail-async-done-count",
            "async",
            "TypeError",
            format!(
                "$DONE() must report exactly once; observed {} completion reports",
                reports.len()
            ),
        );
    }

    let report = reports[0];
    if report.as_str() == ASYNC_COMPLETE {
        return classify_normal(metadata);
    }
    let failure = report
        .strip_prefix(ASYNC_FAILURE_PREFIX)
        .expect("async report prefix was filtered");
    let (actual_type, detail) = failure
        .split_once(": ")
        .map_or(("Test262Error", failure), |(name, detail)| (name, detail));
    WorkerResult::failure("fail-async", "async", actual_type, detail)
}

fn classify_normal(metadata: &Metadata) -> WorkerResult {
    if let Some(negative) = &metadata.negative {
        WorkerResult::failure(
            "fail-missing-throw",
            "normal",
            "",
            format!(
                "expected {} during {}",
                negative.error_type.as_deref().unwrap_or("an exception"),
                negative.phase.as_deref().unwrap_or("any phase")
            ),
        )
    } else {
        WorkerResult::pass("normal", "")
    }
}

fn classify_completion(
    metadata: &Metadata,
    actual_phase: &str,
    diagnostic: &ExceptionDiagnostic,
) -> WorkerResult {
    let Some(negative) = &metadata.negative else {
        return WorkerResult::failure_with_diagnostic(
            format!("fail-{actual_phase}"),
            actual_phase,
            &diagnostic.error_type,
            &diagnostic.message,
            diagnostic.line,
            diagnostic.column,
        );
    };
    let expected_phase = negative.phase.as_deref();
    let phase_matches = match expected_phase {
        None => true,
        Some("parse" | "early") => actual_phase == "parse",
        Some("runtime") => actual_phase == "runtime",
        Some("resolution") => actual_phase == "resolution",
        Some(_) => false,
    };
    let type_matches = negative
        .error_type
        .as_deref()
        .is_none_or(|expected| expected == diagnostic.error_type);
    if phase_matches && type_matches {
        WorkerResult::pass_with_diagnostic(
            actual_phase,
            &diagnostic.error_type,
            &diagnostic.message,
            diagnostic.line,
            diagnostic.column,
        )
    } else {
        WorkerResult::failure_with_diagnostic(
            "fail-negative-mismatch",
            actual_phase,
            &diagnostic.error_type,
            &diagnostic.message,
            diagnostic.line,
            diagnostic.column,
        )
    }
}

fn first_quickjs_stack_source_location(stack: &str) -> Option<(u32, u32)> {
    stack.lines().find_map(|raw_line| {
        let frame = raw_line.trim().strip_prefix("at ")?;
        let location = frame
            .strip_suffix(')')
            .and_then(|frame| frame.rsplit_once(" (").map(|(_, location)| location))
            .unwrap_or(frame);
        let (prefix, column) = location.rsplit_once(':')?;
        let (filename, line) = prefix.rsplit_once(':')?;
        if filename.is_empty() {
            return None;
        }
        let line = line.parse::<u32>().ok().filter(|value| *value > 0)?;
        let column = column.parse::<u32>().ok().filter(|value| *value > 0)?;
        Some((line, column))
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        ExceptionDiagnostic, classify_async_print_log, classify_completion,
        first_quickjs_stack_source_location, run_worker,
    };
    use crate::admissions::sha256;
    use crate::metadata::{Metadata, NegativeExpectation};
    use crate::{Variant, WorkerOptions};

    fn admissions_path() -> PathBuf {
        PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("dev-support/test262/admissions.tsv")
    }

    fn admissions_sha256() -> String {
        sha256(&fs::read(admissions_path()).expect("read checked-in admissions"))
    }

    fn admission_row(fields: [&str; 16]) -> String {
        fields
            .map(|field| if field.is_empty() { "-" } else { field })
            .join("\t")
    }

    fn write_parse_rejected_admissions(
        suite: &Path,
        relative: &Path,
        source: &str,
    ) -> (PathBuf, String) {
        const HEADER: &str = "kind\tgroup\tpath\tsource_sha256\tincludes\tflags\tfeatures\tnegative_phase\tnegative_type\tclosure_file_count\tpriority\trequest_index\tspecifier\tnormalized_path\tpolicy\tcohort";
        let relative = relative.to_str().unwrap();
        let source_sha256 = sha256(source.as_bytes());
        let mut rows = [
            admission_row([
                "dynamic-import-root",
                "parse-rejected",
                relative,
                "",
                "",
                "",
                "",
                "",
                "",
                "1",
                "0",
                "",
                "",
                "",
                "parse-rejected",
                "",
            ]),
            admission_row([
                "graph-file",
                "parse-rejected",
                relative,
                &source_sha256,
                "",
                "raw",
                "dynamic-import",
                "parse",
                "SyntaxError",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
            ]),
        ];
        rows.sort();
        let contents = format!("{HEADER}\n{}\n", rows.join("\n"));
        let path = suite.join("admissions.tsv");
        fs::write(&path, &contents).unwrap();
        (path, sha256(contents.as_bytes()))
    }

    fn dynamic_import_admissions(root_source: &str, fixture_source: &str) -> String {
        const HEADER: &str = "kind\tgroup\tpath\tsource_sha256\tincludes\tflags\tfeatures\tnegative_phase\tnegative_type\tclosure_file_count\tpriority\trequest_index\tspecifier\tnormalized_path\tpolicy\tcohort";
        let root_sha256 = sha256(root_source.as_bytes());
        let fixture_sha256 = sha256(fixture_source.as_bytes());
        let mut rows = [
            admission_row([
                "dynamic-import-root",
                "dynamic-import-worker",
                "test/dynamic.js",
                "",
                "",
                "",
                "",
                "",
                "",
                "2",
                "0",
                "",
                "",
                "",
                "initial-import-tree",
                "",
            ]),
            admission_row([
                "graph-file",
                "dynamic-import-worker",
                "test/dynamic.js",
                &root_sha256,
                "",
                "async,raw",
                "dynamic-import",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
            ]),
            admission_row([
                "graph-file",
                "dynamic-import-worker",
                "test/fixture_FIXTURE.js",
                &fixture_sha256,
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
            ]),
            admission_row([
                "graph-request",
                "dynamic-import-worker",
                "test/dynamic.js",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "",
                "0",
                "./fixture_FIXTURE.js",
                "test/fixture_FIXTURE.js",
                "",
                "",
            ]),
        ];
        rows.sort();
        format!("{HEADER}\n{}\n", rows.join("\n"))
    }

    #[test]
    fn agent_host_worker_flag_revalidates_path_and_source() {
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-agent-worker-revalidation-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let wrong_path = PathBuf::from("test/not-good-views.js");
        fs::create_dir_all(suite.join("test/built-ins/Atomics/wait")).unwrap();
        fs::write(
            suite.join(&wrong_path),
            "/*---\nincludes: [atomicsHelper.js]\nfeatures: [Atomics]\n---*/\n",
        )
        .unwrap();
        let error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: wrong_path,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: true,
        })
        .unwrap_err();
        assert!(error.contains("rejected unaudited path"), "{error}");

        let exact_path = PathBuf::from("test/built-ins/Atomics/wait/good-views.js");
        fs::write(
            suite.join(&exact_path),
            "/*---\nincludes: [atomicsHelper.js]\nfeatures: [Atomics]\n---*/\n// drift\n",
        )
        .unwrap();
        let error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: exact_path,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: true,
        })
        .unwrap_err();
        assert!(error.contains("source drifted"), "{error}");

        let broadcast_path =
            PathBuf::from("test/built-ins/Atomics/notify/notify-with-no-agents-waiting.js");
        fs::create_dir_all(suite.join("test/built-ins/Atomics/notify")).unwrap();
        fs::write(
            suite.join(&broadcast_path),
            "/*---\nincludes: [atomicsHelper.js]\nfeatures: [Atomics, SharedArrayBuffer, TypedArray]\n---*/\n// drift\n",
        )
        .unwrap();
        let error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: broadcast_path,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: true,
        })
        .unwrap_err();

        let bounded_wait_path =
            PathBuf::from("test/built-ins/Atomics/wait/true-for-timeout-agent.js");
        fs::write(
            suite.join(&bounded_wait_path),
            "/*---\nincludes: [atomicsHelper.js]\nfeatures: [Atomics, SharedArrayBuffer, TypedArray]\n---*/\n// drift\n",
        )
        .unwrap();
        let bounded_wait_error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: bounded_wait_path,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: true,
        })
        .unwrap_err();

        let wake_count_location_path =
            PathBuf::from("test/built-ins/Atomics/notify/bigint/notify-all-on-loc.js");
        fs::create_dir_all(suite.join("test/built-ins/Atomics/notify/bigint")).unwrap();
        fs::write(
            suite.join(&wake_count_location_path),
            "/*---\nincludes: [atomicsHelper.js]\nfeatures: [Atomics, BigInt, SharedArrayBuffer, TypedArray]\n---*/\n// drift\n",
        )
        .unwrap();
        let wake_count_location_error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: wake_count_location_path,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: true,
        })
        .unwrap_err();

        let fifo_wake_order_path =
            PathBuf::from("test/built-ins/Atomics/notify/notify-in-order.js");
        fs::write(
            suite.join(&fifo_wake_order_path),
            "/*---\nincludes: [atomicsHelper.js]\nfeatures: [Atomics, SharedArrayBuffer, TypedArray]\n---*/\n// drift\n",
        )
        .unwrap();
        let fifo_wake_order_error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: fifo_wake_order_path,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: true,
        })
        .unwrap_err();
        fs::remove_dir_all(suite).unwrap();
        assert!(
            error.contains("agent broadcast cohort A source drifted"),
            "{error}"
        );
        assert!(
            bounded_wait_error.contains("agent bounded wait cohort A source drifted"),
            "{bounded_wait_error}"
        );
        assert!(
            wake_count_location_error.contains("agent wake/count/location cohort source drifted"),
            "{wake_count_location_error}"
        );
        assert!(
            fifo_wake_order_error.contains("agent FIFO wake-order cohort source drifted"),
            "{fifo_wake_order_error}"
        );
    }

    #[test]
    fn worker_rejects_every_unauthenticated_dynamic_import_test() {
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-dynamic-import-worker-deny-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let relative = PathBuf::from("test/unadmitted-dynamic-import.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        fs::write(
            suite.join(&relative),
            "/*---\nflags: [raw]\nfeatures: [source-phase-imports]\n---*/\nimport('./fixture_FIXTURE.js');\n",
        )
        .unwrap();

        let error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: true,
            allow_agent_host: false,
        })
        .unwrap_err();
        fs::remove_dir_all(suite).unwrap();

        assert!(
            error.contains("dynamic-import Script worker rejected unaudited path"),
            "{error}"
        );
    }

    #[test]
    fn worker_dynamic_import_bytecode_guard_does_not_confuse_import_named_members() {
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-dynamic-import-worker-member-name-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let relative = PathBuf::from("test/import-member-names.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        fs::write(
            suite.join(&relative),
            "/*---\nflags: [raw]\n---*/\nvar object = { get import() { return 1; }, set import(value) {}, import() {} };\nclass C { import() {} static import() {} get import() { return 1; } set import(value) {} }\n",
        )
        .unwrap();

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        });
        fs::remove_dir_all(suite).unwrap();
        let result = result.unwrap();

        assert_eq!(result.outcome, "pass", "{}", result.detail);
    }

    #[test]
    fn dynamic_import_parse_negative_is_classified_before_bytecode_admission() {
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-dynamic-import-worker-parse-negative-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let relative = PathBuf::from("test/invalid-import-call.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        let source = "/*---\nflags: [raw]\nfeatures: [dynamic-import]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\nnew import('');\n";
        fs::write(suite.join(&relative), source).unwrap();
        let (admissions, admissions_sha256) =
            write_parse_rejected_admissions(&suite, &relative, source);

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions,
            admissions_sha256,
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        });
        fs::remove_dir_all(suite).unwrap();
        let result = result.unwrap();

        assert_eq!(result.outcome, "pass", "{}", result.detail);
        assert_eq!(result.actual_phase, "parse");
        assert_eq!(result.actual_type, "SyntaxError");
    }

    #[test]
    fn parse_rejected_dynamic_import_root_must_not_compile() {
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-dynamic-import-worker-parse-success-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let relative = PathBuf::from("test/invalid-import-call.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        let source = "/*---\nflags: [raw]\nfeatures: [dynamic-import]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\n0;\n";
        fs::write(suite.join(&relative), source).unwrap();
        let (admissions, admissions_sha256) =
            write_parse_rejected_admissions(&suite, &relative, source);

        let error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions,
            admissions_sha256,
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        })
        .unwrap_err();
        fs::remove_dir_all(suite).unwrap();

        assert!(error.contains("parse-rejected"), "{error}");
        assert!(error.contains("unexpectedly compiled"), "{error}");
    }

    #[test]
    fn parse_rejected_dynamic_import_root_cannot_fall_through_to_host_policy() {
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-dynamic-import-worker-parse-capability-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let relative = PathBuf::from("test/invalid-import-call.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        let source = "/*---\nflags: [raw]\nfeatures: [dynamic-import]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\nimport('');\n";
        fs::write(suite.join(&relative), source).unwrap();
        let (admissions, admissions_sha256) =
            write_parse_rejected_admissions(&suite, &relative, source);

        let error = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions,
            admissions_sha256,
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        })
        .unwrap_err();
        fs::remove_dir_all(suite).unwrap();

        // The engine compiles `import('')`, so the root is refused before
        // any host policy could stand in for the expected SyntaxError.
        assert!(error.contains("parse-rejected"), "{error}");
        assert!(error.contains("unexpectedly compiled"), "{error}");
    }

    #[test]
    fn authenticated_dynamic_import_root_runs_as_a_relative_named_async_script() {
        const ROOT_SOURCE: &str = "/*---\nflags: [async, raw]\nfeatures: [dynamic-import]\n---*/\nfunction load() { return import(\"./fixture_FIXTURE.js\"); }\nvar dynamicLoad = Function(\"return import({ toString: function () { throw 42; } })\");\ndynamicLoad().then(function() {\n  throw new Error(\"dynamic import unexpectedly fulfilled\");\n}, function(reason) {\n  if (reason !== 42) throw new Error(\"wrong dynamic rejection\");\n  return load();\n}).then(function(ns) {\n  if (ns.value !== 42) throw new Error(\"wrong authored namespace\");\n  $DONE();\n}, $DONE);\n";
        const FIXTURE_SOURCE: &str = "export const value = 42;\n";
        const DONE_HANDLE: &str = "function $DONE(error) { print(error ? 'Test262:AsyncTestFailure:' + error : 'Test262:AsyncTestComplete'); }\n";
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-dynamic-import-worker-allow-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let relative = PathBuf::from("test/dynamic.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        fs::create_dir_all(suite.join("harness")).unwrap();
        fs::write(suite.join(&relative), ROOT_SOURCE).unwrap();
        fs::write(suite.join("test/fixture_FIXTURE.js"), FIXTURE_SOURCE).unwrap();
        fs::write(suite.join("harness/doneprintHandle.js"), DONE_HANDLE).unwrap();
        let admissions = dynamic_import_admissions(ROOT_SOURCE, FIXTURE_SOURCE);
        let admissions_path = suite.join("admissions.tsv");
        fs::write(&admissions_path, &admissions).unwrap();

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path,
            admissions_sha256: sha256(admissions.as_bytes()),
            variant: Variant::Sloppy,
            allow_async_host: true,
            allow_agent_host: false,
        });
        fs::remove_dir_all(suite).unwrap();
        let result = result.unwrap();

        assert_eq!(result.outcome, "pass", "{}", result.detail);
        assert_eq!(result.actual_phase, "normal");
    }

    #[test]
    fn matching_negative_result_preserves_its_diagnostic_provenance() {
        let metadata = Metadata {
            negative: Some(NegativeExpectation {
                phase: Some("parse".to_owned()),
                error_type: Some("SyntaxError".to_owned()),
            }),
            ..Metadata::default()
        };
        let result = classify_completion(
            &metadata,
            "parse",
            &ExceptionDiagnostic {
                error_type: "SyntaxError".to_owned(),
                message: "unexpected token in expression: '}'".to_owned(),
                line: Some(3),
                column: Some(7),
            },
        );

        assert_eq!(result.outcome, "pass");
        assert_eq!(result.actual_phase, "parse");
        assert_eq!(result.actual_type, "SyntaxError");
        assert_eq!(result.detail, "unexpected token in expression: '}'");
        assert_eq!(
            (result.actual_line, result.actual_column),
            (Some(3), Some(7))
        );
    }

    #[test]
    fn quickjs_stack_location_parser_skips_native_frames_and_rejects_partial_coordinates() {
        assert_eq!(
            first_quickjs_stack_source_location(
                "    at nativeCall (native)\n    at <anonymous> (fixture.js:8:45)\n",
            ),
            Some((8, 45))
        );
        assert_eq!(
            first_quickjs_stack_source_location("    at fixture.js:7:50\n"),
            Some((7, 50))
        );
        for stack in [
            "",
            "    at nativeCall (native)\n",
            "    at <anonymous> (fixture.js:0:45)\n",
            "    at <anonymous> (fixture.js:8:0)\n",
            "    at <anonymous> (fixture.js:8)\n",
        ] {
            assert_eq!(
                first_quickjs_stack_source_location(stack),
                None,
                "{stack:?}"
            );
        }
    }

    #[test]
    fn async_completion_requires_exactly_one_done_report() {
        let metadata = Metadata::default();
        let complete = classify_async_print_log(
            &metadata,
            &[
                "ordinary output".to_owned(),
                "Test262:AsyncTestComplete".to_owned(),
            ],
        );
        assert_eq!(complete.outcome, "pass");

        for messages in [
            Vec::<String>::new(),
            vec![
                "Test262:AsyncTestComplete".to_owned(),
                "Test262:AsyncTestComplete".to_owned(),
            ],
        ] {
            let result = classify_async_print_log(&metadata, &messages);
            assert_eq!(result.outcome, "fail-async-done-count");
            assert_eq!(result.actual_type, "TypeError");
        }
    }

    #[test]
    fn async_done_failure_preserves_type_and_message() {
        let result = classify_async_print_log(
            &Metadata::default(),
            &["Test262:AsyncTestFailure:Test262Error: sentinel".to_owned()],
        );
        assert_eq!(result.outcome, "fail-async");
        assert_eq!(result.actual_phase, "async");
        assert_eq!(result.actual_type, "Test262Error");
        assert_eq!(result.detail, "sentinel");
    }

    #[test]
    fn parse_negative_that_compiles_is_not_executed() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-test262-{}-{unique}",
            std::process::id()
        ));
        let relative = PathBuf::from("test/parse-negative.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        fs::write(
            suite.join(&relative),
            "/*---\nflags: [raw]\nnegative:\n  phase: parse\n  type: SyntaxError\n---*/\nthrow 1;\n",
        )
        .unwrap();

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        })
        .unwrap();
        fs::remove_dir_all(suite).unwrap();

        assert_eq!(result.outcome, "fail-missing-throw");
        assert_eq!(result.actual_phase, "normal");
        assert!(result.detail.contains("expected SyntaxError during parse"));
    }

    #[test]
    fn dependency_free_request_shaped_module_negatives_stop_during_parse() {
        let suite = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
            .join("target/oracle/quickjs-2026-06-04/test262");
        if !suite.is_dir() {
            return;
        }

        // Each source contains a static import/export request, but this exact
        // cohort is admitted only as dependency-free parse-negative input.
        // The worker therefore installs no fixture loader and must observe the
        // SyntaxError while compiling, before link or module resolution.
        for relative in [
            "test/language/export/escaped-as-export-specifier.js",
            "test/language/import/dup-bound-names.js",
            "test/language/module-code/early-dup-export-as-star-as.js",
            "test/language/module-code/parse-err-semi-named-export-from.js",
        ] {
            let result = run_worker(&WorkerOptions {
                suite: suite.clone(),
                test: PathBuf::from(relative),
                admissions: admissions_path(),
                admissions_sha256: admissions_sha256(),
                variant: Variant::Sloppy,
                allow_async_host: false,
                allow_agent_host: false,
            })
            .unwrap_or_else(|error| panic!("run {relative}: {error}"));
            assert_eq!(result.outcome, "pass", "{relative}: {}", result.detail);
            assert_eq!(result.actual_phase, "parse", "{relative}");
            assert_eq!(result.actual_type, "SyntaxError", "{relative}");
        }
    }

    #[test]
    fn async_worker_host_is_explicit_and_requires_one_done_report() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-test262-async-host-{}-{unique}",
            std::process::id()
        ));
        let relative = PathBuf::from("test/async-host.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        fs::create_dir_all(suite.join("harness")).unwrap();
        fs::write(
            suite.join("harness/doneprintHandle.js"),
            "function $DONE(error) { print(error ? 'Test262:AsyncTestFailure:Test262Error: failed' : 'Test262:AsyncTestComplete'); }\n",
        )
        .unwrap();
        fs::write(
            suite.join(&relative),
            "/*---\nflags: [async, raw]\n---*/\n$DONE();\n",
        )
        .unwrap();

        let denied = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative.clone(),
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        })
        .unwrap_err();
        assert_eq!(denied, "unsupported test reached worker");

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: true,
            allow_agent_host: false,
        })
        .unwrap();
        fs::remove_dir_all(suite).unwrap();

        assert_eq!(result.outcome, "pass", "{}", result.detail);
    }

    #[test]
    fn async_worker_print_reads_lines_and_keeps_its_log_private() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-test262-async-print-{}-{unique}",
            std::process::id()
        ));
        let relative = PathBuf::from("test/async-print.js");
        fs::create_dir_all(suite.join("test")).unwrap();
        fs::create_dir_all(suite.join("harness")).unwrap();
        fs::write(suite.join("harness/doneprintHandle.js"), "").unwrap();
        fs::write(
            suite.join(&relative),
            "/*---\nflags: [async, raw]\n---*/\nif (typeof globalThis.__quickjs_oxide_test262_print_log__ !== 'undefined') throw new Error('print log is visible');\nprint('ordinary output');\nprint('Test262:AsyncTestComplete');\n",
        )
        .unwrap();

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: true,
            allow_agent_host: false,
        })
        .unwrap();
        fs::remove_dir_all(suite).unwrap();

        assert_eq!(result.outcome, "pass", "{}", result.detail);
    }

    #[test]
    fn raw_worker_installs_print_host_for_coerce_global_style_tests() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-test262-print-{}-{unique}",
            std::process::id()
        ));
        let relative =
            PathBuf::from("test/built-ins/RegExp/prototype/Symbol.replace/coerce-global.js");
        let path = suite.join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"/*---
flags: [raw]
features: [Symbol.replace]
---*/
if (typeof assert !== "undefined") {
    throw new Error("raw worker unexpectedly installed the Test262 harness");
}
if (typeof print !== "function" || print.name !== "print" || print.length !== 1) {
    throw new Error("qjs print host surface is missing");
}
if (print("discarded", 1, true) !== undefined) {
    throw new Error("qjs print host did not return undefined");
}

Array.print = print;
var r = /a/g;
Object.defineProperty(r, "global", { writable: true });
r.lastIndex = 0;
r.global = undefined;
if (r[Symbol.replace]("aa", "b") !== "ba") {
    throw new Error("coerce-global replacement did not complete");
}
"#,
        )
        .unwrap();

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        })
        .unwrap();
        fs::remove_dir_all(suite).unwrap();

        assert_eq!(result.outcome, "pass", "{}", result.detail);
        assert_eq!(result.actual_phase, "normal");
    }

    #[test]
    fn raw_worker_installs_quickjs_test262_hosts() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let suite = std::env::temp_dir().join(format!(
            "quickjs-oxide-test262-code-point-range-{}-{unique}",
            std::process::id()
        ));
        let relative = PathBuf::from("test/harness/code-point-range-host.js");
        let path = suite.join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            r#"/*---
flags: [raw]
---*/
if (typeof $262 !== "object" ||
    typeof $262.createRealm !== "function" ||
    typeof $262.detachArrayBuffer !== "function" ||
    typeof $262.evalScript !== "function" ||
    typeof $262.codePointRange !== "function" ||
    typeof $262.gc !== "function" ||
    $262.global !== globalThis) {
    throw new Error("QuickJS Test262 host surface is missing");
}
var globalDescriptor = Object.getOwnPropertyDescriptor(globalThis, "$262");
var createRealmDescriptor = Object.getOwnPropertyDescriptor($262, "createRealm");
var detachDescriptor = Object.getOwnPropertyDescriptor($262, "detachArrayBuffer");
var evalScriptDescriptor = Object.getOwnPropertyDescriptor($262, "evalScript");
var helperDescriptor = Object.getOwnPropertyDescriptor($262, "codePointRange");
var isHTMLDDADescriptor = Object.getOwnPropertyDescriptor($262, "IsHTMLDDA");
var gcDescriptor = Object.getOwnPropertyDescriptor($262, "gc");
var hostGlobalDescriptor = Object.getOwnPropertyDescriptor($262, "global");
if (!globalDescriptor.writable || !globalDescriptor.enumerable ||
    !globalDescriptor.configurable || !createRealmDescriptor.writable ||
    !createRealmDescriptor.enumerable || !createRealmDescriptor.configurable ||
    !detachDescriptor.writable ||
    !detachDescriptor.enumerable || !detachDescriptor.configurable ||
    !evalScriptDescriptor.writable || !evalScriptDescriptor.enumerable ||
    !evalScriptDescriptor.configurable ||
    !helperDescriptor.writable ||
    !helperDescriptor.enumerable || !helperDescriptor.configurable ||
    !isHTMLDDADescriptor.writable || !isHTMLDDADescriptor.enumerable ||
    !isHTMLDDADescriptor.configurable ||
    !gcDescriptor.writable || !gcDescriptor.enumerable ||
    !gcDescriptor.configurable || !hostGlobalDescriptor.writable ||
    !hostGlobalDescriptor.enumerable || !hostGlobalDescriptor.configurable) {
    throw new Error("QuickJS host property flags changed");
}
if ($262.createRealm.name !== "createRealm" ||
    $262.createRealm.length !== 0 ||
    Object.getPrototypeOf($262.createRealm) !== Function.prototype) {
    throw new Error("QuickJS createRealm function metadata changed");
}
if ($262.detachArrayBuffer.name !== "detachArrayBuffer" ||
    $262.detachArrayBuffer.length !== 1 ||
    Object.getPrototypeOf($262.detachArrayBuffer) !== Function.prototype) {
    throw new Error("QuickJS detachArrayBuffer function metadata changed");
}
if ($262.evalScript.name !== "evalScript" || $262.evalScript.length !== 1 ||
    Object.getPrototypeOf($262.evalScript) !== Function.prototype) {
    throw new Error("QuickJS evalScript function metadata changed");
}
if ($262.codePointRange.name !== "codePointRange" ||
    $262.codePointRange.length !== 2 ||
    Object.getPrototypeOf($262.codePointRange) !== Function.prototype) {
    throw new Error("QuickJS codePointRange function metadata changed");
}
var htmlDDA = isHTMLDDADescriptor.value;
if (htmlDDA !== $262.IsHTMLDDA || htmlDDA.name !== "IsHTMLDDA" ||
    htmlDDA.length !== 0 ||
    Object.getPrototypeOf(htmlDDA) !== Function.prototype ||
    typeof htmlDDA !== "undefined" || Boolean(htmlDDA) || !htmlDDA !== true ||
    (htmlDDA == null) !== true || (null == htmlDDA) !== true ||
    (htmlDDA == undefined) !== true || (undefined == htmlDDA) !== true ||
    htmlDDA === null || htmlDDA === undefined ||
    Object.is(htmlDDA, undefined) || htmlDDA !== htmlDDA ||
    (htmlDDA ?? 42) !== htmlDDA || htmlDDA() !== null) {
    throw new Error("QuickJS IsHTMLDDA semantics changed");
}
var boundHTMLDDA = htmlDDA.bind(null);
var proxyHTMLDDA = new Proxy(htmlDDA, {});
if (typeof boundHTMLDDA !== "function" || !Boolean(boundHTMLDDA) ||
    boundHTMLDDA() !== null || typeof proxyHTMLDDA !== "function" ||
    !Boolean(proxyHTMLDDA) || proxyHTMLDDA() !== null) {
    throw new Error("QuickJS IsHTMLDDA marker propagated to a new object");
}
if ($262.gc.name !== "gc" || $262.gc.length !== 0 ||
    Object.getPrototypeOf($262.gc) !== Function.prototype) {
    throw new Error("QuickJS gc function metadata changed");
}
var constructorThrew = false;
try {
    new $262.createRealm();
} catch (error) {
    constructorThrew = error instanceof TypeError;
}
if (!constructorThrew) {
    throw new Error("QuickJS createRealm became constructible");
}
constructorThrew = false;
try {
    new $262.codePointRange(0, 1);
} catch (error) {
    constructorThrew = error instanceof TypeError;
}
if (!constructorThrew) {
    throw new Error("QuickJS codePointRange became constructible");
}
constructorThrew = false;
try {
    new $262.detachArrayBuffer(null);
} catch (error) {
    constructorThrew = error instanceof TypeError;
}
if (!constructorThrew) {
    throw new Error("QuickJS detachArrayBuffer became constructible");
}
constructorThrew = false;
try {
    new $262.evalScript("0");
} catch (error) {
    constructorThrew = error instanceof TypeError;
}
if (!constructorThrew) {
    throw new Error("QuickJS evalScript became constructible");
}
constructorThrew = false;
try {
    new htmlDDA();
} catch (error) {
    constructorThrew = error instanceof TypeError;
}
if (!constructorThrew) {
    throw new Error("QuickJS IsHTMLDDA became constructible");
}
constructorThrew = false;
try {
    new $262.gc();
} catch (error) {
    constructorThrew = error instanceof TypeError;
}
if (!constructorThrew) {
    throw new Error("QuickJS gc became constructible");
}

var activeResult = (function activeFrame() {
    var local = { answer: 42 };
    activeFrame = null;
    if ($262.gc.call(local, local, "ignored") !== undefined) {
        throw new Error("QuickJS gc return value changed");
    }
    return local.answer;
})();
if (activeResult !== 42) {
    throw new Error("QuickJS gc released an active frame root");
}

var parentJobRan = false;
Promise.resolve().then(function() { parentJobRan = true; });
// NEWCORE-REALMS-001: one realm per runtime.
var realmError;
try {
    $262.createRealm();
} catch (error) {
    realmError = error;
}
if (!(realmError instanceof TypeError) || realmError.message !== "realms are not supported") {
    throw new Error("createRealm did not throw the dropped-feature TypeError");
}
if (parentJobRan) {
    throw new Error("createRealm drained the parent job queue");
}
if ($262.evalScript("21 * 2") !== 42 || $262.evalScript("globalThis") !== globalThis) {
    throw new Error("QuickJS evalScript completion changed");
}
$262.gc();
if (parentJobRan) {
    throw new Error("QuickJS host hooks drained active Promise jobs");
}

var ordinary = {};
if ($262.detachArrayBuffer() !== undefined ||
    $262.detachArrayBuffer(ordinary, "ignored") !== undefined ||
    ordinary.detached !== undefined) {
    throw new Error("QuickJS detachArrayBuffer non-buffer behavior changed");
}
var buffer = new ArrayBuffer(4, { maxByteLength: 8 });
if (buffer.byteLength !== 4 || buffer.maxByteLength !== 8 ||
    !buffer.resizable || buffer.detached) {
    throw new Error("ArrayBuffer pre-detach state changed");
}
// A detached buffer's maxByteLength is engine semantics (quickjs-oxide
// returns 0 as ECMAScript says, QuickJS keeps 8), not the host's concern.
if ($262.detachArrayBuffer.call(ordinary, buffer) !== undefined ||
    buffer.byteLength !== 0 || !buffer.resizable || !buffer.detached) {
    throw new Error("QuickJS detachArrayBuffer state transition changed");
}
if ($262.detachArrayBuffer(buffer) !== undefined || !buffer.detached) {
    throw new Error("QuickJS detachArrayBuffer is no longer idempotent");
}

var conversionLog = "";
var start = Object();
start.valueOf = function() { conversionLog += "s"; return 65.9; };
var end = Object();
end.valueOf = function() { conversionLog += "e"; return 68.9; };
var extra = Object();
extra.valueOf = function() { conversionLog += "x"; throw new Error("extra coerced"); };
if ($262.codePointRange.call(null, start, end, extra) !== "ABC" ||
    conversionLog !== "se") {
    throw new Error("QuickJS codePointRange conversion order changed");
}
var marker = Object();
start.valueOf = function() { conversionLog = "S"; throw marker; };
end.valueOf = function() { conversionLog += "E"; return 68; };
try {
    $262.codePointRange(start, end);
    throw new Error("QuickJS codePointRange swallowed a conversion throw");
} catch (error) {
    if (error !== marker || conversionLog !== "S") {
        throw new Error("QuickJS codePointRange throw order changed");
    }
}
if ($262.codePointRange(4294967361, 68) !== "ABC" ||
    $262.codePointRange(-1, 68) !== "") {
    throw new Error("QuickJS codePointRange ToUint32 behavior changed");
}
var surrogate = $262.codePointRange(0xD7FF, 0xD801);
if (surrogate.length !== 2 || surrogate.charCodeAt(0) !== 0xD7FF ||
    surrogate.charCodeAt(1) !== 0xD800) {
    throw new Error("QuickJS codePointRange surrogate behavior changed");
}
var capped = $262.codePointRange(0x10FFFF, -1);
if (capped.length !== 2 || capped.codePointAt(0) !== 0x10FFFF) {
    throw new Error("QuickJS codePointRange Unicode cap changed");
}
"#,
        )
        .unwrap();

        let result = run_worker(&WorkerOptions {
            suite: suite.clone(),
            test: relative,
            admissions: admissions_path(),
            admissions_sha256: admissions_sha256(),
            variant: Variant::Sloppy,
            allow_async_host: false,
            allow_agent_host: false,
        })
        .unwrap();
        fs::remove_dir_all(suite).unwrap();

        assert_eq!(result.outcome, "pass", "{}", result.detail);
        assert_eq!(result.actual_phase, "normal");
    }
}
