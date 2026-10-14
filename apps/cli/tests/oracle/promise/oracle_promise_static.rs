use crate::runtime_observation::eval;
use quickjs_oxide::engine::api::{Runtime, Value};

const FIXTURE: &str = include_str!("../../fixtures/inputs/r3n_promise_static.js");
const EXPECTED: &str =
    include_str!("../../fixtures/expected/r3n_promise_static.quickjs-2026-06-04.txt");

#[test]
fn promise_try_with_resolvers_and_race_match_pinned_quickjs() {
    let runtime =
        Runtime::new_with_host_services(quickjs_oxide_host::SystemHostServices::default());
    let mut context = runtime.new_context().expect("create context");

    drop(eval(&mut context, FIXTURE));
    runtime.run_gc().unwrap();
    while runtime.is_job_pending().expect("runtime state") {
        assert!(runtime.execute_pending_job().unwrap().executed());
        runtime.run_gc().unwrap();
    }

    let Value::String(transcript) = eval(&mut context, "r3nTranscript.join('\\n') + '\\n'") else {
        panic!("R3n Promise static transcript was not a string");
    };
    assert_eq!(transcript.to_utf8_lossy(), EXPECTED);
}
