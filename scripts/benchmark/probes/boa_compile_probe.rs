//! Boa 0.22 front-end probe for the cross-engine compile matrix.
//!
//! `parse` times `Script::parse` (parser plus scope analysis). `compile` times
//! the same parse followed by `Script::codeblock` (global declaration
//! instantiation plus bytecode generation). Context construction, source I/O
//! and teardown stay outside the timed interval. Output is exactly one line:
//! parse_ns:<integer> or compile_ns:<integer>.
use boa_engine::script::Script;
use boa_engine::{Context, Source};
use std::time::Instant;

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.len() == 2 && arguments[1] == "--version" {
        println!("boa-compile-probe 1");
        return;
    }
    if arguments.len() != 3 {
        eprintln!("usage: boa-compile-probe <parse|compile> FILE");
        std::process::exit(2);
    }
    let metric = &arguments[1];
    let path = &arguments[2];
    if metric != "parse" && metric != "compile" {
        eprintln!("unknown metric: {metric}");
        std::process::exit(2);
    }

    let source = std::fs::read_to_string(path).expect("UTF-8 benchmark source");
    let mut context = Context::default();

    let started = Instant::now();
    let script = Script::parse(Source::from_bytes(source.as_bytes()), None, &mut context)
        .expect("parse frozen script");
    if metric == "compile" {
        let codeblock = script
            .codeblock(&mut context)
            .expect("compile frozen script");
        std::hint::black_box(&codeblock);
    }
    let elapsed = started.elapsed().as_nanos();
    std::hint::black_box(&script);
    println!("{metric}_ns:{elapsed}");
}
