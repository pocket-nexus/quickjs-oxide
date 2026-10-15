// Table-driven runner for the JavaScript fixtures under `oracle/cases/`.
//
// A fixture is a `.js` file. An optional first line `// mode: print` selects
// the comparison mode (default `completion`). Text before the first
// `// case: <name>` marker is a prelude prepended to every case; each marker
// starts one independently evaluated case. A file without markers is a single
// case named after the file. A prelude line `// prelude: <name>` splices in
// the shared prelude `oracle/preludes/<name>.js`.
//
// - `completion`: evaluate prelude + case as a global script in a fresh
//   in-process runtime and compare `return|typeof|String(value)` or
//   `throw|object|name|message` with pinned QuickJS's `std.evalScript`.
// - `completion-name`: like `completion`, but thrown objects compare only
//   their `name`.
// - `completion-array`: like `completion`, but an Array result is rendered
//   `return|array|<String(element)>,...`.
// - `print`: run prelude + case through both `qjs -e` executables and compare
//   exit status, stdout and stderr. Each leading `// flags: <qjs options>`
//   line adds one run of every case with those options.
//
// Set `ORACLE_CASE=<substring>` to run only matching `dir/file/case` names.
use crate::quickjs_array_completion_oracle::observe_array_completion;
use crate::runtime_completion_oracle::{observe_array_aware_eval, observe_eval_completion};
use crate::runtime_observation::host_runtime_context;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Completion,
    CompletionName,
    CompletionArray,
    Print,
}

struct Case {
    name: String,
    source: String,
    mode: Mode,
    flags: Vec<String>,
    known_teardown_leak: bool,
}

fn cases_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/oracle/cases")
}

/// Every directory under `oracle/cases/`, sorted.
pub(crate) fn case_directories() -> Vec<String> {
    let mut names = std::fs::read_dir(cases_root())
        .expect("read oracle/cases")
        .map(|entry| entry.expect("read oracle/cases entry").path())
        .filter(|path| path.is_dir())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn parse_fixture(label: &str, text: &str) -> Vec<Case> {
    let mut lines = text.split_inclusive('\n').peekable();
    let mode = match lines.peek().map(|line| line.trim_end()) {
        Some("// mode: print") => Mode::Print,
        Some("// mode: completion") => Mode::Completion,
        Some("// mode: completion-name") => Mode::CompletionName,
        Some("// mode: completion-array") => Mode::CompletionArray,
        Some(line) if line.starts_with("// mode:") => panic!("{label}: unknown {line:?}"),
        _ => Mode::Completion,
    };
    if lines
        .peek()
        .is_some_and(|line| line.starts_with("// mode:"))
    {
        lines.next();
    }
    let mut variants = Vec::new();
    while let Some(flags) = lines
        .peek()
        .and_then(|line| line.trim_end().strip_prefix("// flags:"))
    {
        assert_eq!(mode, Mode::Print, "{label}: flags need print mode");
        variants.push(
            flags
                .split_whitespace()
                .map(str::to_owned)
                .collect::<Vec<_>>(),
        );
        lines.next();
    }
    if variants.is_empty() {
        variants.push(Vec::new());
    }
    let mut prelude = String::new();
    let mut cases: Vec<(String, String)> = Vec::new();
    for line in lines {
        if let Some(name) = line.trim_end().strip_prefix("// case: ") {
            cases.push((name.to_owned(), String::new()));
        } else if let Some((_, source)) = cases.last_mut() {
            source.push_str(line);
        } else if let Some(name) = line.trim_end().strip_prefix("// prelude: ") {
            let path = cases_root().join(format!("../preludes/{name}.js"));
            let shared = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("{label}: read {}: {error}", path.display()));
            prelude.push_str(shared.trim_end_matches('\n'));
            prelude.push('\n');
        } else {
            prelude.push_str(line);
        }
    }
    if cases.is_empty() {
        cases.push((String::new(), std::mem::take(&mut prelude)));
    }
    let prelude = trim_trailing_comments(&prelude);
    let known_teardown_leak = text.contains("\n// known-issue: teardown-leak");
    let mut parsed = Vec::new();
    for (name, source) in cases {
        let source = trim_trailing_comments(&source);
        let source = if prelude.is_empty() {
            source.to_owned()
        } else {
            format!("{prelude}\n{source}")
        };
        let name = if name.is_empty() {
            label.to_owned()
        } else {
            format!("{label} / {name}")
        };
        for flags in &variants {
            let name = if flags.is_empty() {
                name.clone()
            } else {
                format!("{name} [{}]", flags.join(" "))
            };
            parsed.push(Case {
                name,
                source: source.clone(),
                mode,
                flags: flags.clone(),
                known_teardown_leak,
            });
        }
    }
    parsed
}

/// Drop trailing blank and full-line `//` comment lines, which belong to the
/// next case's heading rather than to this case's source.
fn trim_trailing_comments(source: &str) -> &str {
    let mut source = source.trim_end_matches('\n');
    loop {
        let (head, last) = source.rsplit_once('\n').unwrap_or(("", source));
        let last = last.trim_start();
        if source.is_empty() || !(last.is_empty() || last.starts_with("//")) {
            break;
        }
        source = head.trim_end_matches('\n');
    }
    source
}

/// Collect the `.js` fixtures below `path`, recursively and in sorted order.
fn fixture_files(path: &Path, files: &mut Vec<PathBuf>) {
    let mut entries = std::fs::read_dir(path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
        .map(|entry| entry.expect("read fixture entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for entry in entries {
        if entry.is_dir() {
            fixture_files(&entry, files);
        } else {
            assert_eq!(
                entry.extension().and_then(OsStr::to_str),
                Some("js"),
                "unexpected oracle case file {}",
                entry.display(),
            );
            files.push(entry);
        }
    }
}

fn load_directory(directory: &str) -> Vec<Case> {
    let root = cases_root();
    let mut files = Vec::new();
    fixture_files(&root.join(directory), &mut files);
    let mut cases = Vec::new();
    for path in files {
        let relative = path.strip_prefix(&root).unwrap().with_extension("");
        let label = relative.to_string_lossy().replace('\\', "/");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
        cases.extend(parse_fixture(&label, &text));
    }
    assert!(!cases.is_empty(), "{directory} has no oracle cases");
    cases
}

fn observe_oxide_completion(case: &Case) -> String {
    let (runtime, mut context) = host_runtime_context();
    observe_eval_completion(&runtime, &mut context, &case.source, &case.name)
}

fn observe_process(program: &OsStr, case: &Case) -> String {
    let output = Command::new(program)
        .args(&case.flags)
        .args(["-e", &case.source])
        .output()
        .unwrap_or_else(|error| panic!("run {program:?} for {}: {error}", case.name));
    format!(
        "status: {:?}\nstdout:\n{}stderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    )
}

/// Accept quickjs-oxide's debug runtime-teardown live-heap assertion (exit
/// 101 after the script's own output) for fixtures marked
/// `// known-issue: teardown-leak`; stdout must still match exactly.
fn without_teardown_leak(observation: String) -> String {
    const ASSERTION: &str = "runtime teardown left live heap nodes";
    let Some(stderr_start) = observation.find("stderr:\n") else {
        return observation;
    };
    if !observation.starts_with("status: Some(101)") || !observation.contains(ASSERTION) {
        return observation;
    }
    let (head, stderr) = observation.split_at(stderr_start + "stderr:\n".len());
    let kept = stderr
        .find("[ledger]")
        .or_else(|| stderr.find("\nthread '"))
        .map_or(stderr, |end| &stderr[..end]);
    format!(
        "{}{kept}",
        head.replacen("status: Some(101)", "status: Some(0)", 1)
    )
}

fn observe_quickjs_completion(oracle: &OsStr, case: &Case) -> String {
    let output = crate::quickjs_oracle::observe_completion_output(oracle, &case.source, &case.name);
    output.strip_suffix('\n').unwrap_or(&output).to_owned()
}

/// Reduce `throw|object|<name>|<message>` to `throw|object|<name>`.
fn without_error_message(observation: String) -> String {
    match observation.strip_prefix("throw|object|") {
        Some(rest) => format!(
            "throw|object|{}",
            rest.split('|').next().unwrap_or_default()
        ),
        None => observation,
    }
}

fn compare_case(oracle: &OsStr, case: &Case) -> Option<String> {
    let (oxide, quickjs) = match case.mode {
        Mode::Completion => (
            observe_oxide_completion(case),
            observe_quickjs_completion(oracle, case),
        ),
        Mode::CompletionName => (
            without_error_message(observe_oxide_completion(case)),
            without_error_message(observe_quickjs_completion(oracle, case)),
        ),
        Mode::CompletionArray => {
            let (runtime, mut context) = host_runtime_context();
            (
                observe_array_aware_eval(&runtime, &mut context, &case.source, &case.name),
                observe_array_completion(oracle, &case.source, &case.name),
            )
        }
        Mode::Print => {
            let oxide = observe_process(env!("CARGO_BIN_EXE_qjs").as_ref(), case);
            let oxide = if case.known_teardown_leak {
                without_teardown_leak(oxide)
            } else {
                oxide
            };
            (oxide, observe_process(oracle, case))
        }
    };
    (oxide != quickjs).then(|| {
        format!(
            "{}\nsource: {:?}\noxide:  {oxide:?}\noracle: {quickjs:?}",
            case.name, case.source,
        )
    })
}

/// Run every fixture case in `oracle/cases/<directory>` on both engines.
pub(crate) fn run_case_directory(directory: &str) {
    let oracle = require_qjs_oracle!("oracle cases {directory}");
    let filter = std::env::var("ORACLE_CASE").ok();
    let cases = load_directory(directory)
        .into_iter()
        .filter(|case| {
            filter
                .as_deref()
                .is_none_or(|filter| case.name.contains(filter))
        })
        .collect::<Vec<_>>();
    let workers = std::thread::available_parallelism().map_or(2, |count| count.get().min(4));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut failures = std::thread::scope(|scope| {
        let handles = (0..workers)
            .map(|_| {
                scope.spawn(|| {
                    let mut failures = Vec::new();
                    loop {
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(case) = cases.get(index) else {
                            break failures;
                        };
                        if let Some(failure) = compare_case(&oracle, case) {
                            failures.push((index, failure));
                        }
                    }
                })
            })
            .collect::<Vec<_>>();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("oracle case worker panicked"))
            .collect::<Vec<_>>()
    });
    failures.sort();
    assert!(
        failures.is_empty(),
        "{} of {} oracle case(s) in {directory} differed from pinned QuickJS:\n\n{}",
        failures.len(),
        cases.len(),
        failures
            .into_iter()
            .map(|(_, failure)| failure)
            .collect::<Vec<_>>()
            .join("\n\n"),
    );
}
