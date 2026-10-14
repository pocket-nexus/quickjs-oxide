// Batched QuickJS RegExp probes shared by the lookaround, backreference and
// Unicode-property oracles. `tag` keeps each topic's probe identifiers
// (`__qjo_observe{tag}`, `__qjo{tag}_cases`) so the generated JavaScript stays
// recognisable per topic.
use crate::oracle_cases::{CompileCase, StartMatchCase};
use crate::runtime_completion_oracle::{observe_rust_compile, observe_rust_match};
use crate::runtime_observation::{js_utf16, utf16};
use crate::support::quickjs_indexed_oracle::eval_indexed_plain_lines;
use std::ffi::OsStr;
use std::fmt::Write as _;

/// One match probe: UTF-16 pattern and input, flags, and an optional
/// `lastIndex` start.
pub(crate) struct MatchProbe<'a> {
    pub(crate) pattern: &'a [u16],
    pub(crate) flags: &'a str,
    pub(crate) input: &'a [u16],
    pub(crate) start: Option<usize>,
}

/// Run every match probe in one QuickJS process; each line is `N` or
/// `M|index|length|...` with capture ranges and UTF-16 hex text.
pub(crate) fn run_match_probes(
    oracle: &OsStr,
    tag: &str,
    probes: &[MatchProbe<'_>],
    description: &str,
) -> Vec<String> {
    let set_start = if probes.iter().any(|probe| probe.start.is_some()) {
        "    regexp.lastIndex = test.start;\n"
    } else {
        ""
    };
    let mut source = format!(
        r#"
function __qjo_hex_utf16(value) {{
    var result = "";
    for (var i = 0; i < value.length; i++)
        result += ("0000" + value.charCodeAt(i).toString(16)).slice(-4);
    return result;
}}
function __qjo_observe{tag}(test) {{
    var flags = test.flags;
    if (flags.indexOf("d") < 0) flags += "d";
    if (flags.indexOf("g") < 0 && flags.indexOf("y") < 0) flags += "g";
    var regexp = new RegExp(test.pattern, flags);
{set_start}    var match = regexp.exec(test.input);
    if (match === null) return "N";
    var fields = ["M", String(match.index), String(match.length)];
    for (var i = 0; i < match.length; i++) {{
        if (match[i] === undefined) {{
            fields.push("U");
        }} else {{
            var range = match.indices[i];
            fields.push(String(range[0]) + "," + String(range[1]) + ":" +
                        __qjo_hex_utf16(match[i]));
        }}
    }}
    return fields.join("|");
}}
var __qjo{tag}_cases = [
"#
    );
    for probe in probes {
        let start = probe
            .start
            .map_or_else(String::new, |start| format!(",start:{start}"));
        writeln!(
            source,
            "{{pattern:{},flags:{:?},input:{}{start}}},",
            js_utf16(probe.pattern),
            probe.flags,
            js_utf16(probe.input),
        )
        .unwrap();
    }
    write!(
        source,
        r#"];
for (var __qjo_i = 0; __qjo_i < __qjo{tag}_cases.length; __qjo_i++)
    print(__qjo_i + "|" + __qjo_observe{tag}(__qjo{tag}_cases[__qjo_i]));
"#
    )
    .unwrap();
    eval_indexed_plain_lines(oracle, &source, probes.len(), description)
}

/// Compile every case in one QuickJS process; each line is `ok` or the
/// thrown error rendered by `error_expression` (a JavaScript expression over
/// `error`).
pub(crate) fn run_compile_probes(
    oracle: &OsStr,
    tag: &str,
    cases: &[CompileCase],
    error_expression: &str,
    description: &str,
) -> Vec<String> {
    let mut source = format!("var __qjo{tag}_compile_cases = [\n");
    for case in cases {
        writeln!(
            source,
            "{{pattern:{},flags:{:?}}},",
            js_utf16(&utf16(case.pattern)),
            case.flags,
        )
        .unwrap();
    }
    write!(
        source,
        r#"];
for (var __qjo_i = 0; __qjo_i < __qjo{tag}_compile_cases.length; __qjo_i++) {{
    var result = "ok";
    try {{
        new RegExp(__qjo{tag}_compile_cases[__qjo_i].pattern,
                   __qjo{tag}_compile_cases[__qjo_i].flags);
    }} catch (error) {{
        result = {error_expression};
    }}
    print(__qjo_i + "|" + result);
}}
"#
    )
    .unwrap();
    eval_indexed_plain_lines(oracle, &source, cases.len(), description)
}

fn start_probes(cases: &[StartMatchCase]) -> Vec<MatchProbe<'_>> {
    cases
        .iter()
        .map(|case| MatchProbe {
            pattern: &case.pattern,
            flags: case.flags,
            input: &case.input,
            start: Some(case.start),
        })
        .collect()
}

/// Lookaround suites: `topic` is `lookahead` or `lookbehind`.
pub(crate) fn run_lookaround_match_oracle(
    oracle: &OsStr,
    topic: &str,
    cases: &[StartMatchCase],
) -> Vec<String> {
    run_match_probes(
        oracle,
        &format!("_{topic}"),
        &start_probes(cases),
        &format!("RegExp {topic} matching"),
    )
}

pub(crate) fn run_lookaround_compile_oracle(
    oracle: &OsStr,
    topic: &str,
    cases: &[CompileCase],
) -> Vec<String> {
    run_compile_probes(
        oracle,
        &format!("_{topic}"),
        cases,
        "error.name",
        &format!("RegExp {topic} grammar"),
    )
}

/// Check that the pinned lookaround vectors still complete and compile as
/// frozen.
pub(crate) fn self_check_lookaround(
    oracle: &OsStr,
    topic: &str,
    cases: &[StartMatchCase],
    compile_cases: &[CompileCase],
) {
    let observations = run_lookaround_match_oracle(oracle, topic, cases);
    assert_eq!(observations.len(), cases.len());
    for (case, observation) in cases.iter().zip(&observations) {
        assert!(
            observation == "N" || observation.starts_with("M|"),
            "{topic} oracle vector had no match completion for {}: {observation:?}",
            case.label,
        );
    }

    let observations = run_lookaround_compile_oracle(oracle, topic, compile_cases);
    assert_eq!(observations.len(), compile_cases.len());
    for (case, observation) in compile_cases.iter().zip(&observations) {
        assert_eq!(
            observation, case.expected,
            "pinned QuickJS {topic} grammar changed for {}",
            case.label,
        );
    }
}

/// Compare Rust matching and compilation against pinned QuickJS, collecting
/// every drift under `drift_label`.
pub(crate) fn compare_lookaround(
    oracle: &OsStr,
    topic: &str,
    drift_label: &str,
    cases: &[StartMatchCase],
    compile_cases: &[CompileCase],
) {
    let oracle_observations = run_lookaround_match_oracle(oracle, topic, cases);
    assert_eq!(oracle_observations.len(), cases.len());

    let mut failures = Vec::new();
    for (index, (case, expected)) in cases.iter().zip(&oracle_observations).enumerate() {
        let actual = observe_rust_match(case);
        if actual != *expected {
            failures.push(format!(
                "match case {index}: {}\npattern: {:?}\nflags: {:?}\ninput: {:?}\nstart: {}\noxide: {actual:?}\noracle: {expected:?}",
                case.label, case.pattern, case.flags, case.input, case.start,
            ));
        }
    }

    let oracle_observations = run_lookaround_compile_oracle(oracle, topic, compile_cases);
    for (index, (case, expected)) in compile_cases.iter().zip(&oracle_observations).enumerate() {
        let actual = observe_rust_compile(case);
        if actual != *expected {
            failures.push(format!(
                "compile case {index}: {}\npattern: {:?}\nflags: {:?}\noxide: {actual:?}\noracle: {expected:?}",
                case.label, case.pattern, case.flags,
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{drift_label} drifted in {} case(s):\n\n{}",
        failures.len(),
        failures.join("\n\n"),
    );
}
