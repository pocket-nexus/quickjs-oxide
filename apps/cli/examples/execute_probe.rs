//! Compile once, then time fixed guest work without CLI startup or compilation.
use quickjs_oxide::engine::api::{Runtime, Value};
use quickjs_oxide_host::SystemHostServices;
use std::{env, hint::black_box, time::Instant};

fn main() {
    let case = env::args().nth(1).expect("case name");
    if case == "--version" {
        println!("oxide-c1-execute-probe 1");
        return;
    }
    let (source, expected) = match case.as_str() {
        "primitive" => (
            "(function(){function subtract(a,b){return a-b}var sum=0;for(var i=0;i<1000000;i++)sum+=subtract(true,2);return sum})()",
            Value::Int(-1_000_000),
        ),
        "primitive-frame" => (
            "(function(n,operand){var sum=0;for(var i=0;i<n;i++)sum+=operand-2;return sum})(1000000,true)",
            Value::Int(-1_000_000),
        ),
        "number" => (
            "(function(){function subtract(a,b){return a-b}var sum=0;for(var i=0;i<1000000;i++)sum+=subtract(3,2);return sum})()",
            Value::Int(1_000_000),
        ),
        "object" => (
            "(function(){function subtract(a,b){return a-b}var operand={valueOf(){return 3}},sum=0;for(var i=0;i<100000;i++)sum+=subtract(operand,2);return sum})()",
            Value::Int(100_000),
        ),
        "compare" => (
            "(function(){function compare(o,y){if(o.x<y)return 1;return 2}var operand={x:'1'},sum=0;for(var i=0;i<100000;i++)sum+=compare(operand,2);return sum})()",
            Value::Int(100_000),
        ),
        "m1-hit" => (
            "(function(){function f(n){var a=[1,2,3,4],i=0,j=0,scale=2,sum=0;for(;i<n;i++){j=i&3;sum+=a[j]*scale}return sum}return f(2000000)})()",
            Value::Int(10_000_000),
        ),
        "m1-miss" => (
            "(function(){function f(n){var a=[,],i=0,j=0,scale=2,sum=0;for(;i<n;i++){sum+=a[j]*scale}return Number.isNaN(sum)}return f(2000000)})()",
            Value::Bool(true),
        ),
        _ => panic!("unknown case: {case}"),
    };
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    let mut context = runtime.new_context();
    let compiled = context.compile(source).expect("compile fixed workload");
    for _ in 0..3 {
        assert_eq!(context.execute(&compiled).expect("warmup"), expected);
    }
    for _ in 0..9 {
        let started = Instant::now();
        let actual = context.execute(&compiled).expect("execute fixed workload");
        let elapsed = started.elapsed().as_nanos();
        assert_eq!(actual, expected);
        let _ = black_box(actual);
        println!("execute_ns:{elapsed}");
    }
}
