use crate::runtime_observation::eval;
use crate::runtime_observation::host_runtime_context;
use quickjs_oxide::engine::api::core::Value;

const FIXTURE: &str = include_str!("../../fixtures/inputs/r3p_promise_all.js");
const EXPECTED: &str =
    include_str!("../../fixtures/expected/r3p_promise_all.quickjs-2026-06-04.txt");

#[test]
fn promise_all_matches_pinned_quickjs() {
    let (runtime, mut context) = host_runtime_context();

    drop(eval(&mut context, FIXTURE));
    runtime.run_gc().unwrap();
    while runtime.is_job_pending().expect("runtime state") {
        runtime.run_gc().unwrap();
        assert!(runtime.execute_pending_job().unwrap().executed());
        runtime.run_gc().unwrap();
    }

    let Value::String(transcript) = eval(&mut context, "r3pTranscript.join('\\n') + '\\n'") else {
        panic!("R3p Promise.all transcript was not a string");
    };
    assert_eq!(transcript.to_utf8_lossy(), EXPECTED);
}
