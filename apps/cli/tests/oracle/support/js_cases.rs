// Table-driven runner for the JavaScript fixtures under `oracle/cases/`.
//
// A fixture is a `.js` file. An optional first line `// mode: print` selects
// the comparison mode (default `completion`). Text before the first
// `// case: <name>` marker is a prelude prepended to every case; each marker
// starts one independently evaluated case. A file without markers is a single
// case named after the file. In completion modes a case may hold several
// global scripts separated by `// ---` lines, run in order in one realm. A prelude line `// prelude: <name>` splices in
// the shared prelude `oracle/preludes/<name>.js`.
//
// - `completion`: evaluate prelude + case as a global script in a fresh
//   in-process runtime and compare `return|typeof|String(value)` or
//   `throw|object|name|message` with pinned QuickJS's `std.evalScript`.
// - `completion-name`: like `completion`, but thrown objects compare only
//   their `name`.
// - `completion-array`: like `completion`, but an Array result is rendered
//   `return|array|<String(element)>,...`.
// - `completion-units`: like `completion`, but returned values and thrown
//   primitives render as `<type>|<value>`, keeping signed zero and strings as
//   `<length>|<UTF-16 hex>`.
// - `print`: run prelude + case through both `qjs -e` executables and compare
//   exit status, stdout and stderr. Each leading `// flags: <qjs options>`
//   line adds one run of every case with those options.
//
// Set `ORACLE_CASE=<substring>` to run only matching `dir/file/case` names.
//
// A case named in `dev-support/oracle-attributions.tsv` differs from QuickJS
// by one of the deviation ledger's dropped features: it must still differ, so that a row outlives no difference.
use crate::quickjs_array_completion_oracle::observe_array_completion;
use crate::runtime_completion_oracle::ORACLE_NORMALIZER;
use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::normalize_rust_value_units;
use quickjs_oxide::engine::api::core::{JsString, RuntimeError, Value};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Completion,
    CompletionName,
    CompletionArray,
    CompletionUnits,
    Print,
}

struct Case {
    name: String,
    source: String,
    mode: Mode,
    flags: Vec<String>,
}

/// The attributed cases: name and ledger identifier.
fn attributions() -> std::collections::HashMap<String, String> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dev-support/oracle-attributions.tsv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    text.lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| {
            let (case, deviation) = line
                .split_once('\t')
                .unwrap_or_else(|| panic!("malformed attribution row {line:?}"));
            (case.to_owned(), deviation.to_owned())
        })
        .collect()
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
        Some("// mode: completion-units") => Mode::CompletionUnits,
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

// The same formatting the QuickJS observers apply, evaluated by quickjs-oxide
// after the case so getters and String() conversions run identically.
const OXIDE_FORMATTER: &str = r"(function (kind, value, arrays) {
  if (kind === 'throw' && value !== null && typeof value === 'object')
    return 'throw|object|' + value.name + '|' + value.message;
  if (arrays && kind === 'return' && Array.isArray(value)) {
    var text = '';
    for (var index = 0; index < value.length; index++) {
      if (index) text += ',';
      text += String(value[index]);
    }
    return 'return|array|' + text;
  }
  return kind + '|' + typeof value + '|' + String(value);
})";

/// Separates the scripts of a sequence case; they run one after another as
/// separate global scripts in the same realm, one completion line each.
const SCRIPT_SEPARATOR: &str = "\n// ---\n";

/// Evaluate the case in a fresh runtime and format its completion(s) in JS.
fn observe_oxide_completion(case: &Case, arrays: bool) -> String {
    let (runtime, context) = host_runtime_context();
    let Ok(Value::Object(formatter)) = context.eval(OXIDE_FORMATTER) else {
        panic!("{}: formatter did not evaluate", case.name);
    };
    let formatter = runtime
        .as_callable(&formatter)
        .unwrap()
        .expect("formatter is callable");
    let mut lines = Vec::new();
    for script in case.source.split(SCRIPT_SEPARATOR) {
        let (kind, value) = match context.eval(script) {
            Ok(value) => ("return", value),
            Err(RuntimeError::Exception(exception)) => ("throw", exception),
            Err(error) => panic!("Rust engine failure for {}: {error}", case.name),
        };
        let kind = Value::String(JsString::try_from_utf8(kind).unwrap());
        match context.call(
            &formatter,
            Value::Undefined,
            &[kind, value, Value::Bool(arrays)],
        ) {
            Ok(Value::String(text)) => lines.push(text.to_utf8_lossy()),
            other => panic!("{}: formatting the completion failed: {other:?}", case.name),
        }
    }
    lines.join("\n")
}

/// Like [`observe_oxide_completion`], but returned values and thrown
/// primitives are rendered by `normalize_rust_value_units`.
fn observe_oxide_units(case: &Case) -> String {
    let (runtime, context) = host_runtime_context();
    match context.eval(&case.source) {
        Ok(value) => normalize_rust_value_units(&value),
        Err(RuntimeError::Exception(Value::Object(object)))
            if runtime.as_callable(&object).unwrap().is_none() =>
        {
            observe_oxide_completion(case, false)
        }
        Err(RuntimeError::Exception(value)) => {
            format!("throw|{}", normalize_rust_value_units(&value))
        }
        Err(error) => panic!("Rust engine failure for {}: {error}", case.name),
    }
}

const UNITS_OBSERVER: &str = r"
var __qjo_mode = 0, __qjo_value;
try { __qjo_value = std.evalScript(std.in.readAsString()); }
catch (error) {
  if (error !== null && typeof error === 'object') {
    __qjo_mode = 2;
    print('throw|object|' + error.name + '|' + error.message);
  } else {
    __qjo_mode = 1;
    __qjo_value = error;
    var __qjo_print = print;
    print = function (text) { __qjo_print('throw|' + text); };
  }
}
";

fn observe_quickjs_units(oracle: &OsStr, case: &Case) -> String {
    let bootstrap = format!("{UNITS_OBSERVER}if (__qjo_mode !== 2) {{\n{ORACLE_NORMALIZER}}}\n");
    let output = crate::quickjs_oracle::run_stdin_utf8(
        oracle,
        &bootstrap,
        None,
        &case.source,
        &case.name,
        "units observer",
    );
    output.strip_suffix('\n').unwrap_or(&output).to_owned()
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

// Runs inside a function so the observer declares no globals of its own.
const SEQUENCE_OBSERVER: &str = r"(function () {
var scripts = std.in.readAsString().split('\n// ---\n');
for (var index = 0; index < scripts.length; index++) {
  try {
    var value = std.evalScript(scripts[index]);
    print('return|' + typeof value + '|' + String(value));
  } catch (error) {
    if (error !== null && typeof error === 'object')
      print('throw|object|' + error.name + '|' + error.message);
    else
      print('throw|' + typeof error + '|' + String(error));
  }
}
})();
";

fn observe_quickjs_completion(oracle: &OsStr, case: &Case) -> String {
    let output = crate::quickjs_oracle::run_stdin_utf8(
        oracle,
        SEQUENCE_OBSERVER,
        None,
        &case.source,
        &case.name,
        "completion observer",
    );
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
            observe_oxide_completion(case, false),
            observe_quickjs_completion(oracle, case),
        ),
        Mode::CompletionName => (
            without_error_message(observe_oxide_completion(case, false)),
            without_error_message(observe_quickjs_completion(oracle, case)),
        ),
        Mode::CompletionUnits => (
            observe_oxide_units(case),
            observe_quickjs_units(oracle, case),
        ),
        Mode::CompletionArray => (
            observe_oxide_completion(case, true),
            observe_array_completion(oracle, &case.source, &case.name),
        ),
        Mode::Print => (
            observe_process(env!("CARGO_BIN_EXE_qjs").as_ref(), case),
            observe_process(oracle, case),
        ),
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
    let attributions = attributions();
    let workers = std::thread::available_parallelism().map_or(2, |count| count.get().min(2));
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
                        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            compare_case(&oracle, case)
                        }));
                        let result = match attributions.get(&case.name) {
                            Some(deviation) => result.map(|failure| match failure {
                                Some(_) => None,
                                None => Some(format!(
                                    "{}\nattributed to {deviation} but matches pinned QuickJS: \
                                     remove its row from dev-support/oracle-attributions.tsv",
                                    case.name
                                )),
                            }),
                            None => result,
                        };
                        let failure = result.unwrap_or_else(|panic| {
                            let message = panic
                                .downcast_ref::<String>()
                                .map(String::as_str)
                                .or_else(|| panic.downcast_ref::<&str>().copied())
                                .unwrap_or("non-string panic");
                            Some(format!(
                                "{}\nsource: {:?}\npanicked: {message}",
                                case.name, case.source
                            ))
                        });
                        if let Some(failure) = failure {
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

/// Every row of `dev-support/oracle-attributions.tsv` names a case, once.
pub(crate) fn assert_attributions_name_cases() {
    let names = case_directories()
        .iter()
        .flat_map(|directory| load_directory(directory))
        .map(|case| case.name)
        .collect::<std::collections::HashSet<_>>();
    let mut stale = attributions()
        .into_keys()
        .filter(|case| !names.contains(case))
        .collect::<Vec<_>>();
    stale.sort();
    assert!(stale.is_empty(), "attributions name no case: {stale:#?}");
}
