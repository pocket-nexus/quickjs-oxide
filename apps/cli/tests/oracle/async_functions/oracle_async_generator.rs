//! Differential coverage for ordinary async-generator function declarations and
//! expressions.
//!
//! This milestone deliberately does not claim `yield*`, `for await`, object/class/
//! private method syntax, or `return` crossing an active iterator. Those surfaces
//! need their own implementation milestones and focused oracles.

use crate::runtime_completion_oracle::run;

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
