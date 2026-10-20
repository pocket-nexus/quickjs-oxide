//! Promises and the job queue: running jobs, driving an async function to
//! completion, a job that throws, and unhandled rejections. Backs
//! apps/web/docs/jobs.md.
//!
//! ```sh
//! cargo run --quiet --example jobs
//! ```

use quickjs_oxide::engine::api::core::{
    JsString, ObjectRef, PendingJobOutcome, PromiseState, Runtime, RuntimeError, Value,
};

fn queue() {
    // docs:start queue
    let runtime = Runtime::new();
    let context = runtime.context();

    context
        .eval("globalThis.log = []; Promise.resolve().then(() => log.push('job')); log.push('script');")
        .unwrap();
    // The script has finished; its reaction is still queued.
    assert!(runtime.is_job_pending().unwrap());

    assert_eq!(
        runtime.run_pending_job().unwrap(),
        PendingJobOutcome::Executed
    );
    assert_eq!(runtime.run_pending_job().unwrap(), PendingJobOutcome::NoJob);
    assert_eq!(
        context.eval("log.join()").unwrap(),
        Value::String(JsString::from_static("script,job"))
    );
    // docs:end queue
}

// docs:start drive
/// Runs jobs until `promise` settles; its fulfillment value or rejection
/// reason.
fn settle(runtime: &Runtime, promise: &ObjectRef) -> Result<(PromiseState, Value), RuntimeError> {
    loop {
        let snapshot = runtime
            .promise_snapshot(promise)?
            .ok_or(RuntimeError::Invariant("not a promise"))?;
        if snapshot.state() != PromiseState::Pending {
            return Ok((snapshot.state(), snapshot.result().clone()));
        }
        if !runtime.run_pending_job()?.executed() {
            // Nothing left to run and the promise is still pending.
            return Err(RuntimeError::Invariant("the promise never settles"));
        }
    }
}
// docs:end drive

fn drive() {
    // docs:start drive-call
    let runtime = Runtime::new();
    let context = runtime.context();
    let Value::Object(function) = context
        .eval("(async function (n) { let sum = 0; for (let i = 1; i <= n; i++) sum += await i; return sum; })")
        .unwrap()
    else {
        panic!("expected a function");
    };
    let function = runtime.as_callable(&function).unwrap().unwrap();

    let Value::Object(promise) = context
        .call(&function, Value::Undefined, &[Value::Int(10)])
        .unwrap()
    else {
        panic!("expected a promise");
    };
    assert_eq!(
        settle(&runtime, &promise).unwrap(),
        (PromiseState::Fulfilled, Value::Int(55))
    );
    // docs:end drive-call

    let Value::Object(never) = context.eval("new Promise(() => {})").unwrap() else {
        panic!("expected a promise");
    };
    assert!(matches!(
        settle(&runtime, &never),
        Err(RuntimeError::Invariant(_))
    ));
}

fn throwing_job() {
    // docs:start throwing-job
    let runtime = Runtime::new();
    let context = runtime.context();
    // A FinalizationRegistry callback runs as a job after a collection.
    context
        .eval(
            "globalThis.registry = new FinalizationRegistry((tag) => { throw new TypeError(tag) });
             registry.register({}, 'collected');
             Promise.resolve().then(() => { globalThis.after = true });",
        )
        .unwrap();
    runtime.run_gc().unwrap();

    // The job's exception is pending on the context; the queue keeps the
    // jobs after it.
    let mut thrown = Vec::new();
    loop {
        match runtime.run_pending_job() {
            Ok(PendingJobOutcome::Executed) => {}
            Ok(PendingJobOutcome::NoJob) => break,
            Err(RuntimeError::Exception(exception)) => {
                thrown.push(runtime.print_value(&exception).unwrap());
            }
            Err(other) => panic!("{other}"),
        }
    }
    assert_eq!(thrown.len(), 1);
    assert!(thrown[0].starts_with("TypeError: collected"));
    assert_eq!(context.eval("after").unwrap(), Value::Bool(true));
    // docs:end throwing-job
}

fn rejections() {
    // docs:start rejections
    let runtime = Runtime::new();
    runtime.set_track_rejections(true).unwrap();
    let context = runtime.context();

    context
        .eval(
            "Promise.reject(new Error('lost'));
             const caught = Promise.reject(new Error('caught'));
             caught.catch(() => {});",
        )
        .unwrap();
    runtime.run_jobs().unwrap();

    let unhandled = runtime.take_unhandled_rejections().unwrap();
    assert_eq!(unhandled.len(), 1);
    let (_promise, reason) = &unhandled[0];
    assert!(
        runtime
            .print_value(reason)
            .unwrap()
            .starts_with("Error: lost")
    );
    // The list is emptied by the call.
    assert!(runtime.take_unhandled_rejections().unwrap().is_empty());
    // docs:end rejections
}

fn main() {
    queue();
    drive();
    throwing_job();
    rejections();
    println!("jobs: ok");
}
