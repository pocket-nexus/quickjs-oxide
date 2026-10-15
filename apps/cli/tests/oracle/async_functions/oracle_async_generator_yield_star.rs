//! Differential coverage for `yield*` inside async generators.
//!
//! The observations in this file were captured from pinned QuickJS 2026-06-04.
//! They cover the two different delegation paths: a manually implemented async
//! iterator passes yielded promises through unchanged, while the hidden
//! Async-from-Sync iterator assimilates synchronous iterator values.

use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::{eval, text};

#[test]
fn suspended_delegation_retains_async_and_sync_iterators_across_gc() {
    let (runtime, mut context) = host_runtime_context();
    drop(eval(
        &mut context,
        r#"
var asyncOutcome = "pending";
var syncOutcome = "pending";
var releaseAsync;
var releaseSync;
var asyncGate = new Promise(function (resolve) {
    releaseAsync = resolve;
});
var syncGate = new Promise(function (resolve) {
    releaseSync = resolve;
});
var asyncIterable = {
    [Symbol.asyncIterator]: function () {
        return {
            token: 40,
            next: function () {
                var self = this;
                return asyncGate.then(function () {
                    return { value: self.token + 2, done: false };
                });
            }
        };
    }
};
var syncIterable = {
    [Symbol.iterator]: function () {
        return {
            token: 40,
            next: function () {
                var self = this;
                return {
                    value: syncGate.then(function () {
                        return self.token + 2;
                    }),
                    done: false
                };
            }
        };
    }
};
var asyncIterator = (async function* () {
    yield* asyncIterable;
})();
var syncIterator = (async function* () {
    yield* syncIterable;
})();
asyncIterator.next().then(function (result) {
    asyncOutcome = result.value + ":" + result.done;
});
syncIterator.next().then(function (result) {
    syncOutcome = result.value + ":" + result.done;
});
asyncIterable = null;
syncIterable = null;
asyncIterator = null;
syncIterator = null;
"#,
    ));

    runtime.run_gc().unwrap();
    drop(eval(&mut context, "releaseAsync(); releaseSync();"));
    while runtime.is_job_pending().expect("runtime state") {
        runtime.execute_pending_job().unwrap();
        runtime.run_gc().unwrap();
    }

    assert_eq!(
        text(eval(&mut context, "asyncOutcome + '|' + syncOutcome")),
        "42:false|42:false"
    );
}
