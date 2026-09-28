use std::time::Instant;

use quickjs_oxide::engine::api::Runtime;
use quickjs_oxide_host::SystemHostServices;

fn record(runtime: &Runtime, mode: &str, phase: &str) {
    let snapshot = runtime.memory_snapshot();
    for category in snapshot.categories {
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
            "CATEGORY\t{mode}\t{phase}\t{}\t{count}\t{used}\t{capacity}",
            category.name
        );
    }
}

fn collect(runtime: &Runtime, mode: &str, phase: &str) {
    eprintln!("PROBE_GC_PHASE\t{mode}\t{phase}");
    let start = Instant::now();
    let stats = runtime.run_gc().expect("explicit collection");
    let elapsed_ns = start.elapsed().as_nanos();
    println!(
        "GC\t{mode}\t{phase}\t{elapsed_ns}\t{}\t{}\t{}\t{}\t{}",
        stats.examined_nodes,
        stats.external_root_nodes,
        stats.candidate_nodes,
        stats.cleanup.finalized_var_refs,
        stats.cleanup.finalized_shapes
    );
    record(runtime, mode, phase);
}

fn eval(context: &mut quickjs_oxide::engine::api::Context, source: &str) {
    drop(context.eval(source).expect("evaluate workload phase"));
}

fn main() {
    let mode = std::env::args().nth(1).expect("cell or shape");
    let burst = match mode.as_str() {
        "cell" => {
            "function make(n) { let value = n; return function() { return value; }; } \
             globalThis.keep = []; \
             for (let i = 0; i < 32768; i++) keep.push(make(i));"
        }
        "shape" => {
            "globalThis.keep = []; \
             for (let i = 0; i < 32768; i++) { \
               let object = {}; object['key' + i] = i; keep.push(object); \
             }"
        }
        _ => panic!("expected cell or shape"),
    };
    let regrow = match mode.as_str() {
        "cell" => "for (let i = 0; i < 4096; i++) keep.push(make(i + 50000));",
        "shape" => {
            "for (let i = 0; i < 4096; i++) { \
               let object = {}; object['next' + i] = i; keep.push(object); \
             }"
        }
        _ => unreachable!(),
    };
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    println!(
        "record\tmode\tphase\tname-or-ns\tcount-or-examined\tused-or-roots\tcapacity-or-candidates\tfinalized-cells\tfinalized-shapes"
    );
    record(&runtime, &mode, "initialized");
    let mut context = runtime.new_context();
    eval(&mut context, burst);
    record(&runtime, &mode, "burst-live");
    collect(&runtime, &mode, "burst-rooted-gc");

    eval(
        &mut context,
        "globalThis.small = []; \
         for (let i = 0; i < 32; i++) small.push(keep[i]); \
         globalThis.keep = small; globalThis.small = null;",
    );
    record(&runtime, &mode, "small-roots-before-gc");
    for pass in 1..=3 {
        collect(&runtime, &mode, &format!("small-roots-gc-{pass}"));
    }

    eval(&mut context, regrow);
    record(&runtime, &mode, "regrown-live");
    collect(&runtime, &mode, "regrown-gc");
    eval(
        &mut context,
        "globalThis.keep = null; globalThis.make = null;",
    );
    record(&runtime, &mode, "unrooted-before-gc");
    collect(&runtime, &mode, "unrooted-gc");
    drop(context);
    collect(&runtime, &mode, "context-teardown-gc");
    eprintln!("PROBE_GC_PHASE\t{mode}\truntime-drop");
}
