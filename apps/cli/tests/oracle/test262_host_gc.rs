use quickjs_oxide::engine::api::{
    Context, DescriptorField, OrdinaryPropertyDescriptor, Runtime, RuntimeError, Value,
};

const FIXTURE: &str = include_str!("host_gc_execution_turn.js");
const QUICKJS_2026_06_04: &str =
    include_str!("../fixtures/expected/host_gc_reentrant.quickjs-2026-06-04.txt");

fn eval(context: &mut Context, source: &str) -> Value {
    context.eval(source).unwrap_or_else(|error| {
        if error == RuntimeError::Exception {
            let exception = context
                .take_exception()
                .expect("take exception")
                .expect("exception");
            let message = if let Value::Object(object) = &exception {
                let key = context.runtime().intern_property_key("message").unwrap();
                context.get_property(object, &key).ok()
            } else {
                None
            };
            panic!("unexpected JavaScript exception: {exception:?}; message: {message:?}");
        }
        panic!("unexpected engine error: {error}");
    })
}

fn text(value: Value) -> String {
    let Value::String(value) = value else {
        panic!("expected a string value");
    };
    value.to_utf8_lossy()
}

fn data_property(value: Value) -> OrdinaryPropertyDescriptor {
    OrdinaryPropertyDescriptor {
        value: DescriptorField::Present(value),
        writable: DescriptorField::Present(true),
        enumerable: DescriptorField::Present(true),
        configurable: DescriptorField::Present(true),
        ..OrdinaryPropertyDescriptor::new()
    }
}

fn install_test262_gc(context: &mut Context) {
    let runtime = context.runtime().clone();
    let object_262 = context.new_object().expect("allocate $262 object");
    let gc = context
        .new_test262_gc_function()
        .expect("allocate $262.gc host function");
    let gc_key = runtime.intern_property_key("gc").expect("intern gc key");
    assert!(
        context
            .define_own_property(
                &object_262,
                &gc_key,
                &data_property(Value::Object(gc.as_object().clone())),
            )
            .expect("define $262.gc")
    );
    let object_262_key = runtime
        .intern_property_key("$262")
        .expect("intern $262 key");
    let global = context.global_object().expect("get global object");
    assert!(
        context
            .define_own_property(
                &global,
                &object_262_key,
                &data_property(Value::Object(object_262)),
            )
            .expect("define global $262")
    );
}

fn drain_jobs(runtime: &Runtime, context: &mut Context) {
    let mut jobs = 0usize;
    while runtime.is_job_pending() {
        jobs += 1;
        assert!(jobs <= 64, "host GC fixture did not settle within 64 jobs");
        if let Err(error) = runtime.execute_pending_job() {
            if error.error() == &RuntimeError::Exception {
                panic!("host GC fixture job threw: {:?}", context.take_exception());
            }
            panic!("host GC fixture job failed: {error}");
        }
    }
    assert!(
        jobs > 0,
        "host GC fixture did not enqueue its promised work"
    );
}

#[test]
fn test262_gc_reentry_preserves_execution_turn_roots_and_job_order() {
    let runtime =
        Runtime::new_with_host_services(quickjs_oxide_host::SystemHostServices::default());
    let mut context = runtime.new_context();
    install_test262_gc(&mut context);

    drop(eval(&mut context, FIXTURE));
    assert!(
        runtime.is_job_pending(),
        "$262.gc must leave Promise and finalization jobs queued"
    );
    // The completed eval cleared kept roots. Collection here cannot run jobs;
    // the pending reaction still owns its captured object.
    runtime.run_gc().unwrap();
    assert_eq!(eval(&mut context, "hostGcJobOrder.length"), Value::Int(0));
    drain_jobs(&runtime, &mut context);

    let transcript = text(eval(&mut context, "hostGcTranscript.join('\\n')"));
    let lines: Vec<_> = transcript.lines().collect();
    let pinned: Vec<_> = QUICKJS_2026_06_04.lines().collect();
    // Common host shape and active roots still match the untouched oracle.
    assert_eq!(&lines[..3], &pinned[..3]);
    assert_eq!(lines[3], "weakref-death=false|true|false|false");
    assert_eq!(lines[4], "weakmap-ephemeron=true|false");
    assert_eq!(lines[5], "gc-sync=0");
    assert_eq!(lines.len(), 7);
    let jobs: Vec<_> = lines[6]
        .strip_prefix("job-fifo=")
        .unwrap()
        .split('|')
        .collect();
    // Promise FIFO is fixed; collection timing and finalizer interleaving are
    // governed by the approved target rather than the pinned RC lifetime.
    let promises: Vec<_> = jobs
        .iter()
        .copied()
        .filter(|job| job.starts_with("promise-"))
        .collect();
    assert_eq!(
        promises,
        [
            "promise-before-native",
            "promise-after-native",
            "promise-before-cycle",
            "promise-between-cycle-gcs",
            "promise-after-cycle"
        ]
    );
    for expected in [
        "pending-active:42",
        "finalizer:native-this",
        "finalizer:native-argument",
        "finalizer:cycle",
        "finalizer:pending",
    ] {
        assert_eq!(
            jobs.iter().filter(|job| **job == expected).count(),
            1,
            "{transcript}"
        );
    }
    assert!(jobs.contains(&"pending-released:true"), "{transcript}");
    assert_eq!(jobs.len(), 12, "{transcript}");
    // The fixture's successful derefs kept these targets through its eval.
    // A fresh turn after explicit collection can now observe their death.
    assert_eq!(
        eval(
            &mut context,
            "hostGcImmediateWeakRef.deref() === undefined && hostGcCycleWeakRef.deref() === undefined && hostGcEphemeronValueRef.deref() === undefined"
        ),
        Value::Bool(true)
    );
}

#[test]
fn test262_gc_nested_embedding_entries_share_the_explicit_execution_turn() {
    let runtime =
        Runtime::new_with_host_services(quickjs_oxide_host::SystemHostServices::default());
    let mut context = runtime.new_context();
    install_test262_gc(&mut context);
    runtime.with_execution_turn(|| {
        drop(eval(&mut context, "var scopeRef = new WeakRef({}); var scopeJobs = 0; Promise.resolve().then(() => scopeJobs++);"));
        runtime.run_gc()?;
        assert_eq!(eval(&mut context, "scopeRef.deref() !== undefined"), Value::Bool(true));
        runtime.with_execution_turn(|| {
            drop(eval(&mut context, "$262.gc();"));
            assert_eq!(eval(&mut context, "scopeRef.deref() !== undefined"), Value::Bool(true));
            Ok(())
        })?;
        assert!(runtime.execute_pending_job().is_err());
        assert!(runtime.is_job_pending());
        assert_eq!(eval(&mut context, "scopeJobs"), Value::Int(0));
        Ok(())
    }).unwrap();
    runtime.run_gc().unwrap();
    assert_eq!(
        eval(&mut context, "scopeRef.deref() === undefined"),
        Value::Bool(true)
    );
    drain_jobs(&runtime, &mut context);
    assert_eq!(eval(&mut context, "scopeJobs"), Value::Int(1));
}
