//! Public compile API timing: source I/O, Runtime/Context creation, execution,
//! and teardown are outside the measured interval.
use quickjs_oxide::engine::api::core::Runtime;
use quickjs_oxide_host::SystemHost;
use std::{env, fs, time::Instant};

fn main() {
    let path = env::args().nth(1).expect("source path");
    if path == "--version" {
        println!("oxide-compile-probe 1");
        return;
    }
    let source = fs::read_to_string(&path).expect("UTF-8 benchmark source");
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context().expect("create context");
    let started = Instant::now();
    let function = context
        .compile_with_filename(&source, &path)
        .expect("compile frozen script");
    let elapsed = started.elapsed().as_nanos();
    std::hint::black_box(&function);
    println!("compile_ns:{elapsed}");
}
