use quickjs_oxide::engine::api::{Runtime, profiling::MemorySnapshot};
use quickjs_oxide_host::SystemHostServices;

fn capacity(snapshot: &MemorySnapshot, name: &str) -> usize {
    snapshot
        .categories
        .iter()
        .find(|category| category.name == name)
        .and_then(|category| category.capacity_bytes)
        .unwrap_or(0)
}

fn count(snapshot: &MemorySnapshot, name: &str) -> usize {
    snapshot
        .categories
        .iter()
        .find(|category| category.name == name)
        .and_then(|category| category.count)
        .unwrap_or(0)
}

fn record(runtime: &Runtime, phase: &str) {
    let snapshot = runtime.memory_snapshot();
    println!(
        "{phase}\t{}\t{}\t{}\t{}\t{}\t{}",
        capacity(&snapshot, "arena_slots"),
        capacity(&snapshot, "var_ref_arena_slots"),
        capacity(&snapshot, "shape_arena_slots"),
        capacity(&snapshot, "collection_scratch_peak"),
        count(&snapshot, "var_refs"),
        count(&snapshot, "shapes")
    );
}

fn main() {
    let mode = std::env::args().nth(1).expect("cell or shape");
    let source = match mode.as_str() {
        "cell" => {
            "function make(n) { let value = n; return function() { return value; }; } \
             globalThis.keep = []; for (let i = 0; i < 4096; i++) keep.push(make(i));"
        }
        "shape" => {
            "globalThis.keep = []; for (let i = 0; i < 4096; i++) { \
             let object = {}; object['key' + i] = i; keep.push(object); }"
        }
        _ => panic!("expected cell or shape"),
    };
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    record(&runtime, "initialized");
    let mut context = runtime.new_context();
    drop(context.eval(source).expect("evaluate workload"));
    record(&runtime, "live");
    runtime.run_gc().expect("collect rooted graph");
    record(&runtime, "rooted_gc");
    drop(context.eval("globalThis.keep = null;").expect("drop roots"));
    record(&runtime, "roots_dropped");
    runtime.run_gc().expect("collect dropped graph");
    record(&runtime, "collected");
    drop(context.eval(source).expect("reuse slots"));
    runtime.run_gc().expect("collect reused graph");
    record(&runtime, "reused");
    drop(context);
    runtime.run_gc().expect("collect after context teardown");
    record(&runtime, "teardown");
}
