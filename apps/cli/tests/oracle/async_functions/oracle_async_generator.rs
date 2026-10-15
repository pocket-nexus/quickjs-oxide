//! Differential coverage for ordinary async-generator function declarations and
//! expressions.
//!
//! This milestone deliberately does not claim `yield*`, `for await`, object/class/
//! private method syntax, or `return` crossing an active iterator. Those surfaces
//! need their own implementation milestones and focused oracles.

use crate::object_graph_observation::{define_context_global as define_global, object};
use crate::runtime_completion_oracle::run;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{eval, text};
use quickjs_oxide::engine::api::Value;

#[test]
fn completed_state_services_one_trailing_request_per_driver_entry_like_quickjs() {
    const SOURCE: &str = r#"
var release;
var gate = new Promise(function (resolve) {
    release = resolve;
});
async function* queued() {
    var value = await gate;
    var resumed = yield value;
    return Promise.resolve(value + 2);
}
var iterator = queued();
var requests = [
    iterator.next(),
    iterator.next(100),
    iterator.next(200),
    iterator.next(300)
];
var settled = [false, false, false, false];
requests.forEach(function (request, index) {
    request.then(function () {
        settled[index] = true;
    });
});
requests[2].then(function (result) {
    Promise.resolve().then(function () {
        print(
            settled.join(",") + "|" +
            String(result.value) + ":" + result.done
        );
        var fifthSettled = false;
        var fifth = iterator.next();
        fifth.then(function () {
            fifthSettled = true;
        });
        requests[3].then(function (trailingResult) {
            Promise.resolve().then(function () {
                print(
                    settled.join(",") + "," + fifthSettled + "|" +
                    String(trailingResult.value) + ":" +
                    trailingResult.done
                );
            });
        });
    });
});
release(40);
"#;
    const EXPECTED_STDOUT: &str = concat!(
        "true,true,true,false|undefined:true\n",
        "true,true,true,true,false|undefined:true\n",
    );

    let oxide = run(env!("CARGO_BIN_EXE_qjs").as_ref(), SOURCE);
    assert!(
        oxide.status.success(),
        "quickjs-oxide rejected completed-state queue probe: {}\nsource:\n{}",
        String::from_utf8_lossy(&oxide.stderr),
        SOURCE
    );
    assert_eq!(
        String::from_utf8_lossy(&oxide.stdout),
        EXPECTED_STDOUT,
        "quickjs-oxide completed-state driver entry drifted from pinned QuickJS"
    );

    if let Some(oracle) = std::env::var_os("QJS_ORACLE") {
        let quickjs = run(&oracle, SOURCE);
        assert!(
            quickjs.status.success(),
            "pinned QuickJS rejected completed-state queue probe: {}",
            String::from_utf8_lossy(&quickjs.stderr)
        );
        assert_eq!(quickjs.stdout, oxide.stdout);
    }
}

#[test]
fn repeated_poisoned_promise_awaits_use_a_bounded_native_stack() {
    const SOURCE: &str = r#"
var promise = Promise.resolve(0);
Object.defineProperty(promise, "constructor", {
    get: function () { throw 0; }
});
async function* repeated() {
    var caught = 0;
    for (var index = 0; index < 20000; index++) {
        try {
            await promise;
        } catch (error) {
            caught++;
        }
    }
    yield caught;
}
repeated().next().then(function (result) {
    print(result.value + ":" + result.done);
});
"#;
    let oxide = run(env!("CARGO_BIN_EXE_qjs").as_ref(), SOURCE);
    assert!(
        oxide.status.success(),
        "quickjs-oxide overflowed or rejected repeated immediate await rejection: {}",
        String::from_utf8_lossy(&oxide.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&oxide.stdout), "20000:false\n");
}

#[test]
fn cross_realm_settlement_resumes_in_the_job_realm_and_executes_in_the_body_realm() {
    let runtime = host_runtime();
    let mut body = runtime.new_context().expect("create context");
    let mut settler = runtime.new_context().expect("create context");
    let settler_realm = settler.realm_id();

    let gate = object(eval(
        &mut settler,
        r#"
var release;
var gate = new Promise(function (resolve) {
    release = resolve;
});
gate
"#,
    ));
    define_global(&mut body, "foreignGate", Value::Object(gate));
    drop(eval(
        &mut body,
        r#"
var crossRealmReason;
async function* crossRealm() {
    await foreignGate;
    throw new TypeError("callee");
}
crossRealm().next().then(undefined, function (error) {
    crossRealmReason = error;
});
"#,
    ));

    // PromiseResolve in the body realm first installs the thenable bridge.
    while runtime.is_job_pending().expect("runtime state") {
        runtime.execute_pending_job().unwrap();
    }
    drop(eval(&mut settler, "release(1)"));

    let mut saw_settler_realm = false;
    while runtime.is_job_pending().expect("runtime state") {
        let job_realm = runtime
            .execute_pending_job()
            .unwrap()
            .context()
            .expect("pending job had no realm");
        saw_settler_realm |= job_realm == settler_realm;
    }
    assert!(
        saw_settler_realm,
        "foreign Promise settlement did not carry its actual job realm"
    );

    let reason = object(eval(&mut body, "crossRealmReason"));
    let body_type_error_prototype = object(eval(&mut body, "TypeError.prototype"));
    assert_eq!(
        runtime.get_prototype_of(&reason).unwrap(),
        Some(body_type_error_prototype),
        "the resumed async-generator body did not retain its defining realm"
    );
    assert_eq!(text(eval(&mut body, "crossRealmReason.message")), "callee");
}
