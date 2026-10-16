//! Phase 1, M5 and M6: Rust-driven micro-workloads for the new heap.
//!
//! `workloads <name> <iterations> [--cap-mib N] [--min-heap-mib N] [--ring N] [--report]`
//!
//! Each workload does fixed work per iteration and prints a checksum, so
//! instruction counts at N and 2N give the per-iteration slope with setup
//! excluded. `--report` prints collector statistics, fragmentation and peak
//! RSS on stderr.
//!
//! | name | per iteration |
//! | --- | --- |
//! | `empty` | the loop and the checksum only |
//! | `constructor` | new object with no reserved slots, four property adds |
//! | `literal` | new object with four reserved slots, four property adds |
//! | `alloc-only` | new object with four reserved slots, no property |
//! | `closures` | one scope (parent: a fixed outer scope) and one function holding it |
//! | `proto-read` | uncached read of a property found three prototypes up |
//! | `proto-read-cached` | the phase 3 IC check: shape compare, validity cell, holder slot load |
//! | `churn` | allocate an object or an array of 1–64 elements into a ring of 4096 live entries (`--ring`) |

use std::hint::black_box;
use std::time::Instant;

use oxide_core::heap::gc::HeapLimits;
use oxide_core::heap::object::ObjectKind;
use oxide_core::{Heap, Intrinsic, ObjectId, PropertyFlags, PropertyKey, Runtime, Value};

const DEFAULT: PropertyFlags = PropertyFlags::DEFAULT;

struct Options {
    name: String,
    iterations: u64,
    cap: Option<usize>,
    min_heap: Option<usize>,
    ring: u32,
    report: bool,
}

fn options() -> Options {
    let mut args = std::env::args().skip(1);
    let name = args.next().expect("workload name");
    let iterations = args
        .next()
        .expect("iterations")
        .parse()
        .expect("iteration count");
    let mut options = Options {
        name,
        iterations,
        cap: None,
        min_heap: None,
        ring: 4096,
        report: false,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cap-mib" => options.cap = Some(args.next().unwrap().parse::<usize>().unwrap() << 20),
            "--min-heap-mib" => {
                options.min_heap = Some(args.next().unwrap().parse::<usize>().unwrap() << 20)
            }
            "--ring" => options.ring = args.next().unwrap().parse().unwrap(),
            "--report" => options.report = true,
            other => panic!("unknown option {other}"),
        }
    }
    options
}

fn keys(heap: &mut Heap) -> [PropertyKey; 4] {
    ["a", "b", "c", "d"].map(|name| heap.property_key(name).unwrap())
}

/// The latest object lives in a root-stack slot, as a register would.
fn allocation(runtime: &mut Runtime, iterations: u64, capacity: u32, adds: usize) -> i64 {
    let prototype = runtime.intrinsic(Intrinsic::ObjectPrototype);
    let heap = runtime.heap_mut();
    let keys = keys(heap);
    let register = heap.roots().len();
    heap.roots().push(Value::UNDEFINED);
    for i in 0..iterations {
        let object = heap
            .alloc_object(ObjectKind::Ordinary, Some(prototype), capacity)
            .unwrap();
        heap.roots().set(register, Value::object(object));
        let value = Value::int32(i as i32);
        for &key in &keys[..adds] {
            heap.add_property(object, key, DEFAULT, value).unwrap();
        }
    }
    let last = heap.roots().get(register).as_object().unwrap();
    i64::from(heap.get_slot(last, 0).as_int32().unwrap_or(-1))
}

fn closures(runtime: &mut Runtime, iterations: u64) -> i64 {
    let outer = runtime.new_scope(None, 1).unwrap();
    runtime.hold(Value::object(outer.object()));
    let prototype = runtime.intrinsic(Intrinsic::FunctionPrototype);
    let heap = runtime.heap_mut();
    let register = heap.roots().len();
    heap.roots().push(Value::UNDEFINED);
    for i in 0..iterations {
        let scope = heap.alloc_scope(Some(outer), 2).unwrap();
        heap.scope_set(scope, 0, 0, Value::int32(i as i32));
        let function = heap.alloc_function(Some(prototype), Some(scope)).unwrap();
        heap.roots().set(register, Value::object(function));
    }
    let function = heap.roots().get(register).as_object().unwrap();
    let scope = heap.function_scope(function).unwrap();
    i64::from(heap.scope_get(scope, 0, 0).as_int32().unwrap())
}

/// receiver -> p1 -> p2 -> holder, `x` on the holder.
fn chain(runtime: &mut Runtime) -> (ObjectId, ObjectId, PropertyKey) {
    let heap = runtime.heap_mut();
    let x = heap.property_key("x").unwrap();
    let holder = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    heap.handles().create(Value::object(holder));
    heap.add_property(holder, x, DEFAULT, Value::int32(3))
        .unwrap();
    let mut current = holder;
    for _ in 0..3 {
        current = heap
            .alloc_object(ObjectKind::Ordinary, Some(current), 4)
            .unwrap();
        heap.handles().create(Value::object(current));
    }
    (current, holder, x)
}

fn proto_read(runtime: &mut Runtime, iterations: u64) -> i64 {
    let (receiver, _, x) = chain(runtime);
    let heap = runtime.heap();
    let mut sum = 0i64;
    for _ in 0..iterations {
        let value = black_box(heap)
            .get_property(black_box(receiver), x)
            .unwrap();
        sum += i64::from(value.as_int32().unwrap());
    }
    sum
}

fn proto_read_cached(runtime: &mut Runtime, iterations: u64) -> i64 {
    let (receiver, holder, x) = chain(runtime);
    let heap = runtime.heap_mut();
    // What a prototype IC records on its first miss.
    let shape = heap.shape_of(receiver);
    let cell = heap.validity_cell(receiver).unwrap();
    let mut walk = receiver;
    while heap.lookup_own(walk, x).is_none() {
        walk = heap.prototype_of(walk).unwrap();
    }
    assert_eq!(walk, holder);
    let (slot, _) = heap.lookup_own(holder, x).unwrap();
    let heap = &*heap;
    let mut sum = 0i64;
    for _ in 0..iterations {
        let heap = black_box(heap);
        let receiver = black_box(receiver);
        let value = if heap.header(receiver).shape == shape && heap.cell_is_valid(cell) {
            heap.get_slot(holder, slot)
        } else {
            heap.get_property(receiver, x).unwrap()
        };
        sum += i64::from(value.as_int32().unwrap());
    }
    sum
}

fn churn(runtime: &mut Runtime, iterations: u64, ring_len: u32) -> i64 {
    let ring = runtime.new_array(ring_len).unwrap();
    runtime.hold(Value::object(ring));
    let prototype = runtime.intrinsic(Intrinsic::ObjectPrototype);
    let array_prototype = runtime.intrinsic(Intrinsic::ArrayPrototype);
    let heap = runtime.heap_mut();
    let keys = keys(heap);
    let mut seed: u32 = 0x2545_F491;
    let mut sum = 0i64;
    for i in 0..iterations {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let value = if seed & 1 == 0 {
            let object = heap
                .alloc_object(ObjectKind::Ordinary, Some(prototype), 2)
                .unwrap();
            heap.add_property(object, keys[0], DEFAULT, Value::int32(i as i32))
                .unwrap();
            heap.add_property(object, keys[1], DEFAULT, Value::int32(1))
                .unwrap();
            Value::object(object)
        } else {
            let len = 1 + (seed >> 8) % 64;
            let array = heap.alloc_array(Some(array_prototype), len).unwrap();
            heap.set_element(array, len - 1, Value::int32(i as i32))
                .unwrap();
            Value::object(array)
        };
        let slot = (i % u64::from(ring_len)) as u32;
        let old = heap.get_element(ring, slot);
        if let Some(old) = old.as_object() {
            sum += i64::from(heap.header(old).kind == ObjectKind::Array);
        }
        heap.set_element(ring, slot, value).unwrap();
    }
    sum
}

fn peak_rss_kib() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn main() {
    let options = options();
    let limits = HeapLimits {
        max_bytes: options.cap.unwrap_or(usize::MAX),
    };
    let mut runtime = Runtime::new(limits).unwrap();
    runtime.heap_mut().gc_mut().record_pauses = options.report;
    if let Some(min_heap) = options.min_heap {
        runtime.heap_mut().gc_mut().min_heap = min_heap;
        runtime.heap_mut().collect();
    }
    let start = Instant::now();
    let n = options.iterations;
    let checksum = match options.name.as_str() {
        "empty" => (0..n).map(|i| black_box(i as i64) & 1).sum(),
        "constructor" => allocation(&mut runtime, n, 0, 4),
        "literal" => allocation(&mut runtime, n, 4, 4),
        "alloc-only" => allocation(&mut runtime, n, 4, 0),
        "closures" => closures(&mut runtime, n),
        "proto-read" => proto_read(&mut runtime, n),
        "proto-read-cached" => proto_read_cached(&mut runtime, n),
        "churn" => churn(&mut runtime, n, options.ring),
        other => panic!("unknown workload {other}"),
    };
    let elapsed = start.elapsed();
    println!("{checksum}");
    if options.report {
        let heap = runtime.heap();
        let gc = heap.gc();
        let stats = gc.stats;
        let mut pauses: Vec<_> = gc
            .pauses
            .iter()
            .map(|pause| pause.as_secs_f64() * 1e6)
            .collect();
        pauses.sort_by(f64::total_cmp);
        let quantile = |q: f64| {
            if pauses.is_empty() {
                0.0
            } else {
                pauses[((pauses.len() - 1) as f64 * q) as usize]
            }
        };
        let allocated_mb = (stats.freed_bytes as f64 + heap.used_bytes() as f64) / 1e6;
        eprintln!(
            "workload {} iterations {n} time {:.3} s",
            options.name,
            elapsed.as_secs_f64()
        );
        eprintln!(
            "collections {} total pause {:.1} ms max {:.0} us p50 {:.0} us p99 {:.0} us",
            stats.collections,
            stats.total_pause.as_secs_f64() * 1e3,
            stats.max_pause.as_secs_f64() * 1e6,
            quantile(0.5),
            quantile(0.99)
        );
        eprintln!(
            "allocated {:.1} MB throughput {:.1} MB/s freed {:.1} MB",
            allocated_mb,
            allocated_mb / elapsed.as_secs_f64(),
            stats.freed_bytes as f64 / 1e6
        );
        let values = heap.value_heap();
        eprintln!(
            "heap reserved {:.2} MiB used {:.2} MiB value slots {} free {} fragmentation {:.3}",
            heap.bytes() as f64 / (1 << 20) as f64,
            heap.used_bytes() as f64 / (1 << 20) as f64,
            values.total_slots(),
            values.free_slots(),
            values.fragmentation()
        );
        eprintln!(
            "live objects {} shapes {}",
            heap.live_objects(),
            heap.live_shapes()
        );
        runtime.heap_mut().collect();
        let heap = runtime.heap();
        let values = heap.value_heap();
        eprintln!(
            "after a final collection: value slots {} used {} free {} (free/total {:.3}), live objects {}",
            values.total_slots(),
            values.used_slots(),
            values.free_slots(),
            values.fragmentation(),
            heap.live_objects()
        );
        if let Some(rss) = peak_rss_kib() {
            eprintln!("peak RSS {:.1} MiB", rss as f64 / 1024.0);
        }
    }
}
