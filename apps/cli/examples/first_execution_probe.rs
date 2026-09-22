//! One first execution of one newly compiled Script in a fresh Runtime.
//! Compile and execute intervals are independent; no subtraction is used.
use quickjs_oxide::engine::api::Runtime;
use quickjs_oxide_host::SystemHostServices;
use std::{env, fs, io::Write, time::Instant};

fn main() {
    let mut args = env::args().skip(1);
    let path = args.next().expect("source path");
    if path == "--version" {
        println!("oxide-first-execution-probe 1");
        return;
    }
    let metrics_path = args.next().expect("new metrics JSON path");
    assert!(args.next().is_none(), "unexpected probe arguments");
    let source = fs::read_to_string(&path).expect("UTF-8 frozen script");
    // Create before timing and never overwrite another sample's receipt.
    let mut metrics = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(metrics_path)
        .expect("create new metrics file");
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    let mut context = runtime.new_context();
    context.install_qjs_helpers().expect("install qjs helpers");
    let compile_started = Instant::now();
    let function = context.compile_with_filename(&source, &path);
    let compile_ns = compile_started.elapsed().as_nanos();
    let function = function.expect("compile frozen script once");

    // This includes first-use executable projections and nested eval compile,
    // host calls/output and execution-time GC. It excludes compile above,
    // source I/O, Runtime/Context/helpers initialization, jobs and teardown.
    let execute_started = Instant::now();
    let completion = context.execute(&function);
    let first_execute_ns = execute_started.elapsed().as_nanos();
    let completion = completion.expect("execute frozen script once");
    std::hint::black_box(&completion);

    // The runner admits only synchronous workloads. Draining outside the
    // interval makes an accidental async workload observable and ineligible.
    let mut pending_jobs = 0_u64;
    while runtime
        .execute_pending_job()
        .expect("drain pending job outside first execution interval")
        .executed()
    {
        pending_jobs += 1;
    }
    writeln!(
        metrics,
        "{{\"schema\":\"oxide-first-execution-probe.v1\",\"compile_ns\":{compile_ns},\"first_execute_ns\":{first_execute_ns},\"compile_count\":1,\"execute_count\":1,\"pending_jobs\":{pending_jobs},\"profiling\":{}}}",
        cfg!(feature = "profiling")
    )
    .expect("write metrics");
}
