//! Differential and lifetime coverage for `for await ... of`.
//!
//! The fixture's QuickJS 2026-06-04 transcript records its non-standard close
//! (QUICKJS-SPEC-016): a completed async iterator's `value` is read and the
//! iterator closed. quickjs-oxide follows ForIn/OfBodyEvaluation (14.7.5.6),
//! whose transcript is `r3ak_for_await_of.spec.txt`; otherwise they agree:
//! ordinary abrupt exits do not await `return()`, and next/result failures
//! occur while the iterator unwind record is disabled.

use crate::runtime_observation::host_runtime_context;
use std::ffi::OsStr;
use std::path::PathBuf;

use std::process::{Command, Output};

use crate::runtime_observation::{eval, text};

const EXPECTED: &str =
    include_str!("../fixtures/expected/r3ak_for_await_of.quickjs-2026-06-04.txt");
const SPECIFIED: &str = include_str!("../fixtures/expected/r3ak_for_await_of.spec.txt");

#[test]
fn for_await_of_transcript_follows_the_specification_where_quickjs_departs() {
    let fixture = fixture_path();
    let oxide = run_file(env!("CARGO_BIN_EXE_qjs").as_ref(), &fixture);
    assert_success("quickjs-oxide", &oxide);
    assert_eq!(
        String::from_utf8_lossy(&oxide.stdout),
        SPECIFIED,
        "quickjs-oxide for-await transcript drifted"
    );

    let oracle = require_pinned_qjs_oracle!("for-await differential");
    let quickjs = run_file(&oracle, &fixture);
    assert_success("pinned QuickJS", &quickjs);
    assert_eq!(
        String::from_utf8_lossy(&quickjs.stdout),
        EXPECTED,
        "pinned QuickJS transcript differs from the frozen fixture"
    );
    assert_ne!(
        oxide.stdout, quickjs.stdout,
        "QUICKJS-SPEC-016 is attributed but quickjs-oxide matches pinned QuickJS"
    );
}

#[test]
fn pending_for_await_next_record_survives_repeated_gc() {
    let (runtime, mut context) = host_runtime_context();
    drop(eval(
        &mut context,
        r#"
var release;
var outcome = "pending";
var generatorRelease;
var generatorFirst = "pending";
var generatorReturn = "pending";
var gate = new Promise(function (resolve) {
    release = resolve;
});
var generatorGate = new Promise(function (resolve) {
    generatorRelease = resolve;
});
var source = {
    [Symbol.asyncIterator]: function () {
        return {
            token: 42,
            next: function () {
                var self = this;
                return gate.then(function () {
                    return { value: self.token, done: false };
                });
            }
        };
    }
};
(async function () {
    for await (var value of source) {
        outcome = String(value);
        break;
    }
})().then(
    function () { outcome += ":done"; },
    function (error) { outcome = error.name + ":" + error.message; }
);
source = null;
gate = null;

var generatorSource = {
    [Symbol.asyncIterator]: function () {
        return {
            token: 43,
            next: function () {
                var self = this;
                return generatorGate.then(function () {
                    return { value: self.token, done: false };
                });
            },
            return: function () {
                return Promise.resolve({});
            }
        };
    }
};
var generatorIterator = (async function* () {
    for await (var value of generatorSource) {
        yield value;
    }
})();
generatorIterator.next().then(function (result) {
    generatorFirst = result.value + ":" + result.done;
});
generatorIterator.return(8).then(function (result) {
    generatorReturn = result.value + ":" + result.done;
});
generatorSource = null;
generatorIterator = null;
generatorGate = null;
"#,
    ));

    for _ in 0..3 {
        runtime.run_gc().unwrap();
    }
    drop(eval(&mut context, "release(); generatorRelease();"));
    while runtime.is_job_pending().expect("runtime state") {
        assert!(runtime.execute_pending_job().unwrap().executed());
        runtime.run_gc().unwrap();
    }
    assert_eq!(
        text(eval(
            &mut context,
            "outcome + '|' + generatorFirst + '|' + generatorReturn"
        )),
        "42:done|43:false|8:true"
    );
}

fn fixture_path() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
        .join("apps/cli/tests/fixtures/inputs/r3ak_for_await_of.js")
}

fn run_file(executable: &OsStr, fixture: &PathBuf) -> Output {
    Command::new(executable)
        .arg(fixture)
        .output()
        .unwrap_or_else(|error| panic!("could not run {executable:?}: {error}"))
}

fn assert_success(engine: &str, output: &Output) {
    assert!(
        output.status.success(),
        "{engine} rejected for-await probe: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
