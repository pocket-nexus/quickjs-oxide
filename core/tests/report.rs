//! Phase 6, section 2.4: the memory report reads the heap without a
//! collection, its reserved total is what the cap compares against, and its
//! live counts follow a known object graph exactly. Passes under
//! `--features gc-stress`.

use oxide_core::{Runtime, RuntimeOptions, Value};

#[test]
fn the_report_matches_the_heap_and_a_known_graph() {
    let mut runtime = Runtime::new(RuntimeOptions::default()).unwrap();
    runtime.run_gc();
    let before = runtime.memory_report();
    assert_eq!(before.reserved_bytes(), runtime.heap().bytes());
    let mut handles = Vec::new();
    for _ in 0..500 {
        let object = runtime.new_object().unwrap();
        handles.push(runtime.hold(Value::object(object)));
    }
    let array = runtime.new_array(8).unwrap();
    handles.push(runtime.hold(Value::object(array)));
    let report = runtime.memory_report();
    // The report itself never collects.
    assert_eq!(report.gc.collections, runtime.heap().gc().stats.collections);
    runtime.run_gc();
    let after = runtime.memory_report();
    assert_eq!(after.objects.live, before.objects.live + 501);
    assert_eq!(after.reserved_bytes(), runtime.heap().bytes());
    assert!(after.values.used_slots >= before.values.used_slots + 8);
    for handle in handles {
        runtime.release(handle);
    }
    runtime.run_gc();
    let freed = runtime.memory_report();
    assert_eq!(freed.objects.live, before.objects.live);
    assert_eq!(freed.reserved_bytes(), runtime.heap().bytes());
}

#[test]
fn options_reach_the_heap_and_the_stack() {
    let options = RuntimeOptions {
        min_heap: 1 << 20,
        growth: 3.0,
        stack_limit: 4096,
        ..RuntimeOptions::default()
    };
    let runtime = Runtime::new(options).unwrap();
    assert_eq!(runtime.heap().gc().min_heap, 1 << 20);
    assert_eq!(runtime.heap().gc().growth_factor, 3.0);
    assert_eq!(runtime.memory_report().threshold, 1 << 20);
    assert_eq!(RuntimeOptions::default().stack_limit, 1 << 20);
}
