use std::time::Instant;

use quickjs_oxide::engine::api::Runtime;
use quickjs_oxide_host::SystemHostServices;

fn record(runtime: &Runtime, mode: &str, scenario: &str, phase: &str) {
    for category in runtime.memory_snapshot().categories {
        let count = category
            .count
            .map_or_else(|| "-".to_owned(), |n| n.to_string());
        let used = category
            .used_bytes
            .map_or_else(|| "-".to_owned(), |n| n.to_string());
        let capacity = category
            .capacity_bytes
            .map_or_else(|| "-".to_owned(), |n| n.to_string());
        println!(
            "CATEGORY\t{mode}\t{scenario}\t{phase}\t{}\t{count}\t{used}\t{capacity}",
            category.name
        );
    }
}

fn collect(runtime: &Runtime, mode: &str, scenario: &str, phase: &str) {
    record(runtime, mode, scenario, &format!("{phase}-before"));
    eprintln!("PROBE_GC_PHASE\t{mode}\t{scenario}\t{phase}");
    let start = Instant::now();
    let stats = runtime.run_gc().expect("explicit collection");
    println!(
        "GC\t{mode}\t{scenario}\t{phase}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
        start.elapsed().as_nanos(),
        stats.examined_nodes,
        stats.external_root_nodes,
        stats.candidate_nodes,
        stats.cleanup.finalized_objects,
        stats.cleanup.finalized_shapes,
        stats.cleanup.finalized_var_refs,
        stats.cleanup.finalized_contexts,
        stats.cleanup.finalized_function_bytecodes,
        stats.cleanup.finalized_strings,
        stats.cleanup.finalized_bigints,
    );
    record(runtime, mode, scenario, phase);
}

fn eval(context: &mut quickjs_oxide::engine::api::Context, source: &str) {
    drop(context.eval(source).expect("evaluate workload phase"));
}

fn populate(
    context: &mut quickjs_oxide::engine::api::Context,
    mode: &str,
    count: usize,
    salt: usize,
) {
    let source = match mode {
        "cell" => format!(
            "if (globalThis.make === undefined) globalThis.make = function(n) {{ let value = n; return function() {{ return value; }}; }}; \
             globalThis.keep = []; for (let i = 0; i < {count}; i++) keep.push(make(i + {salt}));"
        ),
        "shape" => format!(
            "globalThis.keep = []; for (let i = 0; i < {count}; i++) {{ \
               let object = {{}}; object['key' + (i + {salt})] = i; keep.push(object); }}"
        ),
        _ => unreachable!(),
    };
    eval(context, &source);
}

fn reduce_to_32(context: &mut quickjs_oxide::engine::api::Context) {
    eval(
        context,
        "globalThis.small = []; for (let i = 0; i < 32; i++) small.push(keep[i]); \
         globalThis.keep = small; globalThis.small = null;",
    );
}

fn run_post_burst(
    runtime: &Runtime,
    context: &mut quickjs_oxide::engine::api::Context,
    mode: &str,
) {
    let scenario = "post-burst";
    populate(context, mode, 32768, 0);
    record(runtime, mode, scenario, "burst-live");
    collect(runtime, mode, scenario, "burst-rooted-gc");
    reduce_to_32(context);
    record(runtime, mode, scenario, "small-roots-before-gc");
    for pass in 1..=3 {
        collect(runtime, mode, scenario, &format!("small-roots-gc-{pass}"));
    }
    match mode {
        "cell" => eval(
            context,
            "for (let i = 0; i < 4096; i++) keep.push(make(i + 50000));",
        ),
        "shape" => eval(
            context,
            "for (let i = 0; i < 4096; i++) { let object = {}; \
             object['next' + i] = i; keep.push(object); }",
        ),
        _ => unreachable!(),
    }
    record(runtime, mode, scenario, "regrown-live");
    collect(runtime, mode, scenario, "regrown-gc");
    eval(context, "globalThis.keep = null; globalThis.make = null;");
    record(runtime, mode, scenario, "unrooted-before-gc");
    collect(runtime, mode, scenario, "unrooted-gc");
}

fn run_repeated_burst(
    runtime: &Runtime,
    context: &mut quickjs_oxide::engine::api::Context,
    mode: &str,
) {
    let scenario = "repeated-burst";
    for pass in 1..=3 {
        populate(context, mode, 32768, pass * 100000);
        record(runtime, mode, scenario, &format!("burst-{pass}-live"));
        collect(runtime, mode, scenario, &format!("burst-{pass}-rooted-gc"));
        reduce_to_32(context);
        collect(runtime, mode, scenario, &format!("burst-{pass}-small-gc"));
        eval(context, "globalThis.keep = null;");
        collect(
            runtime,
            mode,
            scenario,
            &format!("burst-{pass}-unrooted-gc"),
        );
    }
    eval(context, "globalThis.make = null;");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args.next().expect("cell or shape");
    let scenario = args.next().expect("post-burst or repeated-burst");
    assert!(matches!(mode.as_str(), "cell" | "shape"));
    assert!(matches!(scenario.as_str(), "post-burst" | "repeated-burst"));
    assert!(args.next().is_none());

    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    println!("record\tmode\tscenario\tphase\tfields");
    record(&runtime, &mode, &scenario, "initialized");
    let mut context = runtime.new_context();
    match scenario.as_str() {
        "post-burst" => run_post_burst(&runtime, &mut context, &mode),
        "repeated-burst" => run_repeated_burst(&runtime, &mut context, &mode),
        _ => unreachable!(),
    }
    drop(context);
    collect(&runtime, &mode, &scenario, "context-teardown-gc");
    eprintln!("PROBE_GC_PHASE\t{mode}\t{scenario}\truntime-drop");
}
