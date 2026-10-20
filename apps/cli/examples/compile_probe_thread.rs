//! `compile_probe` on a thread with a declared 512 MiB stack, so the
//! parser's stack guard holds under valgrind (callgrind, dhat). The measured
//! interval and the output line are the same.
use quickjs_oxide::engine::api::{core::Runtime, declare_thread_stack};
use quickjs_oxide_host::SystemHost;
use std::{env, fs, thread, time::Instant};

const STACK_SIZE: usize = 512 << 20;

fn main() {
    let path = env::args().nth(1).expect("source path");
    if path == "--version" {
        println!("oxide-compile-probe-thread 1");
        return;
    }
    let source = fs::read_to_string(&path).expect("UTF-8 benchmark source");
    let elapsed = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || {
            declare_thread_stack(STACK_SIZE);
            let runtime = Runtime::new_with_host_services(SystemHost::default());
            let context = runtime.new_context().expect("create context");
            let started = Instant::now();
            let function = context
                .compile_with_filename(&source, &path)
                .expect("compile frozen script");
            let elapsed = started.elapsed().as_nanos();
            std::hint::black_box(&function);
            elapsed
        })
        .expect("spawn compile thread")
        .join()
        .expect("compile thread");
    println!("compile_ns:{elapsed}");
}
