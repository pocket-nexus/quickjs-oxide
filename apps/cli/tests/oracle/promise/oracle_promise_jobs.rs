use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::integer;
use crate::runtime_observation::text;
use crate::runtime_observation::{drain, eval};
use quickjs_oxide::engine::api::core::{CoreValue, Host, RejectionEvent, Runtime, RuntimeOptions};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn promise_constructor_and_internal_functions_have_quickjs_shapes() {
    let (_runtime, mut context) = host_runtime_context();
    let facts = text(eval(
        &mut context,
        r#"
var resolveFunction;
var rejectFunction;
new Promise(function (resolve, reject) {
    resolveFunction = resolve;
    rejectFunction = reject;
});
var capabilityExecutor;
function CustomPromise(executor) {
    capabilityExecutor = executor;
    executor(function () {}, function () {});
}
Promise.resolve.call(CustomPromise, 1);
[
    typeof Promise,
    Promise.length,
    Promise.name,
    Object.getOwnPropertyNames(resolveFunction).join(','),
    resolveFunction.length,
    resolveFunction.name,
    Object.getPrototypeOf(resolveFunction) === Function.prototype,
    Object.prototype.hasOwnProperty.call(resolveFunction, 'prototype'),
    Object.getOwnPropertyNames(rejectFunction).join(','),
    capabilityExecutor.length,
    capabilityExecutor.name,
    Object.getPrototypeOf(capabilityExecutor) === Function.prototype,
    Object.prototype.hasOwnProperty.call(capabilityExecutor, 'prototype')
].join('|');
"#,
    ));
    assert_eq!(
        facts,
        "function|1|Promise|length,name|1||true|false|length,name|2||true|false"
    );
}

#[test]
fn eval_does_not_drain_and_execute_pending_job_is_fifo_one_at_a_time() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        text(eval(
            &mut context,
            r#"
var order = [];
var settled = Promise.resolve(0);
settled.then(function () {
    order.push('A');
    settled.then(function () { order.push('nested'); });
});
settled.then(function () { order.push('B'); });
order.join('|');
"#,
        )),
        ""
    );
    assert!(runtime.is_job_pending().expect("runtime state"));
    assert!(runtime.execute_pending_job().unwrap().executed());
    assert_eq!(text(eval(&mut context, "order.join('|')")), "A");
    assert!(runtime.execute_pending_job().unwrap().executed());
    assert_eq!(text(eval(&mut context, "order.join('|')")), "A|B");
    assert!(runtime.execute_pending_job().unwrap().executed());
    assert_eq!(text(eval(&mut context, "order.join('|')")), "A|B|nested");
    assert!(!runtime.is_job_pending().expect("runtime state"));
    assert!(!runtime.execute_pending_job().unwrap().executed());
}

#[test]
fn promise_chains_thenables_rejections_and_self_resolution() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        text(eval(
            &mut context,
            r#"
var events = [];
var thenable = {};
Object.defineProperty(thenable, 'then', {
    get: function () {
        events.push('get');
        return function (resolve, reject) {
            events.push('call');
            resolve(40);
            reject(99);
        };
    }
});
var answer = 0;
Promise.resolve(thenable)
    .then(function (value) { events.push('first:' + value); return value + 2; })
    .then(function (value) { answer = value; events.push('answer:' + value); });
var selfResolve;
var self = new Promise(function (resolve) { selfResolve = resolve; });
var selfError = '';
self.then(undefined, function (error) { selfError = error.name; });
selfResolve(self);
events.join('|');
"#,
        )),
        "get"
    );
    assert!(drain(&runtime) >= 4);
    assert_eq!(integer(eval(&mut context, "answer")), 42);
    assert_eq!(text(eval(&mut context, "selfError")), "TypeError");
    assert_eq!(
        text(eval(&mut context, "events.join('|')")),
        "get|call|first:40|answer:42"
    );
}

#[test]
fn queued_jobs_retain_their_graph_across_gc() {
    let (runtime, mut context) = host_runtime_context();
    drop(eval(
        &mut context,
        r#"
var gcAnswer = 0;
(function () {
    Promise.resolve(41).then(function (value) { gcAnswer = value + 1; });
})();
"#,
    ));
    runtime.run_gc().unwrap();
    assert!(drain(&runtime) >= 1);
    assert_eq!(integer(eval(&mut context, "gcAnswer")), 42);
}

#[test]
fn static_identity_catch_and_species_follow_quickjs() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        text(eval(
            &mut context,
            r#"
class DerivedPromise extends Promise {}
class SpeciesPromise extends Promise {
    static get [Symbol.species]() { return Promise; }
}
var base = Promise.resolve(1);
var derived = new DerivedPromise(function (resolve) { resolve(2); });
var speciesSource = new SpeciesPromise(function (resolve) { resolve(3); });
var speciesResult = speciesSource.then(function (value) { return value; });
var catchArgs = '';
var receiver = {
    then: function (fulfilled, rejected) {
        catchArgs = String(fulfilled) + ':' + rejected;
        return 42;
    }
};
var catchResult = Promise.prototype.catch.call(receiver, 'reject-handler');
[
    Promise.resolve(base) === base,
    DerivedPromise.resolve(derived) === derived,
    Promise.resolve(derived) !== derived,
    derived.then(function (value) { return value; }) instanceof DerivedPromise,
    speciesResult instanceof Promise,
    speciesResult instanceof SpeciesPromise,
    catchResult,
    catchArgs
].join('|');
"#,
        )),
        // QuickJS gives `true` for the fourth: a subclass instance's `then`
        // returns a subclass instance. TARGET-SPECIES-001 drops
        // `Symbol.species`, so it returns a base `Promise`.
        "true|true|true|false|true|false|42|undefined:reject-handler"
    );
    drain(&runtime);
}

/// `HostPromiseRejectionTracker` through the core's host hook: an unhandled
/// rejection reports `Rejected`, a handler attached later `Handled`, and the
/// tracked list keeps only what stays unhandled.
#[derive(Default)]
struct RejectionRecorder(Rc<RefCell<Vec<RejectionEvent>>>);

impl Host for RejectionRecorder {
    fn promise_rejection(&mut self, event: RejectionEvent, _promise: CoreValue) {
        self.0.borrow_mut().push(event);
    }
}

#[test]
fn host_rejection_tracker_reports_unhandled_then_late_handled() {
    let recorder = RejectionRecorder::default();
    let events = Rc::clone(&recorder.0);
    let runtime = Runtime::with_host(recorder, RuntimeOptions::default());
    runtime.set_track_rejections(true).unwrap();
    let mut context = runtime.context();
    drop(eval(
        &mut context,
        r#"
var trackerOrder = [];
var late = Promise.reject('late');
late.then(undefined, function () { trackerOrder.push('original'); });
var rejectEarly;
var early = new Promise(function (_, reject) { rejectEarly = reject; });
early.then(undefined, function () {});
rejectEarly('early');
var lost = Promise.reject('lost');
"#,
    ));
    assert_eq!(
        events.borrow().as_slice(),
        [
            RejectionEvent::Rejected,
            RejectionEvent::Handled,
            RejectionEvent::Rejected,
        ]
    );
    drain(&runtime);
    let unhandled = runtime.take_unhandled_rejections().unwrap();
    assert_eq!(unhandled.len(), 1);
    assert_eq!(text(unhandled[0].1.clone()), "lost");
    assert_eq!(
        text(eval(&mut context, "trackerOrder.join('|')")),
        "original"
    );
}
