//! Compiler front-end allocation counter.
//!
//! The measurement boundary matches `apps/cli/examples/compile_probe.rs`:
//! source I/O, Runtime/Context construction and teardown are outside the
//! counted window. This probe is instrumented on purpose, so it is not part
//! of the cross-engine timing contract and `compile_matrix.py` deliberately
//! rejects it. Output is `compile_ns:<ns>` followed by one line per counter;
//! byte counters are requested sizes, `peak_live_bytes` is the high-water mark
//! of outstanding allocations observed during the compile call.
use quickjs_oxide::engine::api::Runtime;
use quickjs_oxide_host::SystemHostServices;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, time::Instant};

struct CountingAllocator;

static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static REALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static DEALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

impl CountingAllocator {
    fn record_allocation(size: usize) {
        ALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        ALLOC_BYTES.fetch_add(size, Ordering::Relaxed);
        let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
        PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
    }

    fn record_release(size: usize) {
        let _ = LIVE_BYTES.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |live| {
            Some(live.saturating_sub(size))
        });
    }

    fn reset() {
        ALLOC_CALLS.store(0, Ordering::Relaxed);
        ALLOC_BYTES.store(0, Ordering::Relaxed);
        REALLOC_CALLS.store(0, Ordering::Relaxed);
        REALLOC_BYTES.store(0, Ordering::Relaxed);
        DEALLOC_CALLS.store(0, Ordering::Relaxed);
        LIVE_BYTES.store(0, Ordering::Relaxed);
        PEAK_LIVE_BYTES.store(0, Ordering::Relaxed);
    }
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc(layout) };
        if !pointer.is_null() {
            CountingAllocator::record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let pointer = unsafe { System.alloc_zeroed(layout) };
        if !pointer.is_null() {
            CountingAllocator::record_allocation(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        DEALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
        CountingAllocator::record_release(layout.size());
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            REALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            REALLOC_BYTES.fetch_add(new_size, Ordering::Relaxed);
            if new_size >= layout.size() {
                CountingAllocator::record_allocation(new_size - layout.size());
            } else {
                CountingAllocator::record_release(layout.size() - new_size);
            }
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn main() {
    let path = env::args().nth(1).expect("source path");
    if path == "--version" {
        println!("oxide-compile-alloc-probe 1");
        return;
    }
    let source = fs::read_to_string(&path).expect("UTF-8 benchmark source");
    let runtime = Runtime::new_with_host_services(SystemHostServices::default());
    let mut context = runtime.new_context();
    CountingAllocator::reset();
    let started = Instant::now();
    let function = context
        .compile_with_filename(&source, &path)
        .expect("compile frozen script");
    let elapsed = started.elapsed().as_nanos();
    let alloc_calls = ALLOC_CALLS.load(Ordering::Relaxed);
    let alloc_bytes = ALLOC_BYTES.load(Ordering::Relaxed);
    let realloc_calls = REALLOC_CALLS.load(Ordering::Relaxed);
    let realloc_bytes = REALLOC_BYTES.load(Ordering::Relaxed);
    let dealloc_calls = DEALLOC_CALLS.load(Ordering::Relaxed);
    let peak_live_bytes = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
    std::hint::black_box(&function);
    println!("compile_ns:{elapsed}");
    println!("alloc_count:{alloc_calls}");
    println!("alloc_bytes:{alloc_bytes}");
    println!("realloc_count:{realloc_calls}");
    println!("realloc_bytes:{realloc_bytes}");
    println!("dealloc_count:{dealloc_calls}");
    println!("peak_live_bytes:{peak_live_bytes}");
}
