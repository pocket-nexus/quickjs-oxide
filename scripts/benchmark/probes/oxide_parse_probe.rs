//! Complete production parsing; I/O and result teardown are outside the timer.
use quickjs_oxide::engine::api::testing::parse_script_for_benchmark;
use std::{env, fs, time::Instant};

fn main() {
    let path = env::args().nth(1).expect("source path");
    if path == "--version" {
        println!("oxide-parse-probe 1");
        return;
    }
    let source = fs::read_to_string(path).expect("UTF-8 benchmark source");
    let started = Instant::now();
    let parsed = parse_script_for_benchmark(&source).expect("parse frozen Script");
    let elapsed = started.elapsed().as_nanos();
    std::hint::black_box(&parsed);
    println!("parse_ns:{elapsed}");
}
