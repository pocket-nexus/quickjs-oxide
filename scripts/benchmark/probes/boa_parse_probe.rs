//! Boa's complete Script grammar without its separate scope-analysis pass.
//! In 0.22, parse_eval(false) invokes exactly ScriptParser::new(false), as does
//! parse_script_with_source before analyze_scope. No engine/VM is constructed.
use boa_interner::Interner;
use boa_parser::{Parser, Source};
use std::{env, fs, time::Instant};

fn main() {
    let path = env::args().nth(1).expect("source path");
    if path == "--version" {
        println!("boa-parse-probe 1 (boa_parser 0.22.0, annex-b)");
        return;
    }
    let source = fs::read_to_string(path).expect("UTF-8 benchmark source");
    let started = Instant::now();
    let mut interner = Interner::default();
    let mut parser = Parser::new(Source::from_bytes(source.as_bytes()));
    let parsed = parser.parse_eval(false, &mut interner).expect("parse frozen Script");
    let elapsed = started.elapsed().as_nanos();
    std::hint::black_box((&parsed, &interner, &parser));
    println!("parse_ns:{elapsed}");
}
