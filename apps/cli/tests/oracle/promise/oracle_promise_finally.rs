use crate::runtime_observation::eval;
use crate::runtime_observation::host_runtime_context;
use quickjs_oxide::engine::api::core::Value;

const FIXTURE: &str = include_str!("../../fixtures/inputs/r3o_promise_finally.js");
const EXPECTED: &str =
    include_str!("../../fixtures/expected/r3o_promise_finally.quickjs-2026-06-04.txt");

#[test]
fn promise_finally_matches_pinned_quickjs() {
    let (runtime, mut context) = host_runtime_context();

    drop(eval(&mut context, FIXTURE));
    runtime.run_gc().unwrap();
    while runtime.is_job_pending().expect("runtime state") {
        runtime.run_gc().unwrap();
        assert!(runtime.execute_pending_job().unwrap().executed());
        runtime.run_gc().unwrap();
    }

    let Value::String(transcript) = eval(&mut context, "r3oTranscript.join('\\n') + '\\n'") else {
        panic!("R3o Promise.finally transcript was not a string");
    };
    // The fixture differs from pinned QuickJS by NEWCORE-SPECIES-001 (its
    // gate row in dev-support/quickjs-fixture-gates.tsv): the
    // `species-abrupt` check expects `finally` to read `constructor` and
    // `@@species`, fails, and ends the transcript after its first line.
    let transcript = transcript.to_utf8_lossy();
    assert_eq!(transcript.lines().next(), EXPECTED.lines().next());
    assert_ne!(
        transcript, EXPECTED,
        "the species deviation no longer shows"
    );
}
