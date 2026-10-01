//! Diagnostic allocation probe for the public Error channel.
//!
//! Build with build_compile_probe.py --probe. This instrumented binary must
//! never supply a formal benchmark Score. Run base/candidate builds with the
//! same iteration count and compare stdout. Linux VmHWM reports process RSS.
use quickjs_oxide::engine::api::{Error, ErrorKind, RuntimeError};
use std::alloc::{GlobalAlloc, Layout, System};
use std::hint::black_box;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

struct CountingAllocator;

static ALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static ALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOC_CALLS: AtomicUsize = AtomicUsize::new(0);
static REALLOC_BYTES: AtomicUsize = AtomicUsize::new(0);
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
        CountingAllocator::record_release(layout.size());
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_pointer = unsafe { System.realloc(pointer, layout, new_size) };
        if !new_pointer.is_null() {
            REALLOC_CALLS.fetch_add(1, Ordering::Relaxed);
            REALLOC_BYTES.fetch_add(new_size, Ordering::Relaxed);
            if new_size >= layout.size() {
                let delta = new_size - layout.size();
                let live = LIVE_BYTES.fetch_add(delta, Ordering::Relaxed) + delta;
                PEAK_LIVE_BYTES.fetch_max(live, Ordering::Relaxed);
            } else {
                CountingAllocator::record_release(layout.size() - new_size);
            }
        }
        new_pointer
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[inline(never)]
fn propagate_error() -> Result<(), RuntimeError> {
    let result: Result<(), Error> = Err(Error::new(ErrorKind::Type, "probe failure"));
    result.map_err(RuntimeError::from)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mode = args
        .next()
        .expect("mode: success|construct|propagate|retain");
    let iterations: usize = args
        .next()
        .expect("iteration count")
        .parse()
        .expect("integer iteration count");
    assert!(iterations > 0);
    assert!(args.next().is_none(), "unexpected argument");
    assert!(
        matches!(
            mode.as_str(),
            "success" | "construct" | "propagate" | "retain"
        ),
        "unsupported mode"
    );

    // Reserve outside the measured window so `retain` isolates Error payloads.
    let mut retained = if mode == "retain" {
        Vec::with_capacity(iterations)
    } else {
        Vec::new()
    };
    CountingAllocator::reset();
    let started = Instant::now();
    for _ in 0..iterations {
        match mode.as_str() {
            "success" => {
                let _ = black_box(Ok::<(), Error>(()));
            }
            "construct" => {
                let _ = black_box(Error::new(ErrorKind::Type, "probe failure"));
            }
            "propagate" => {
                let _ = black_box(propagate_error());
            }
            "retain" => retained.push(Error::new(ErrorKind::Type, "probe failure")),
            _ => unreachable!(),
        }
    }
    black_box(&retained);
    let elapsed_ns = started.elapsed().as_nanos();
    let alloc_calls = ALLOC_CALLS.load(Ordering::Relaxed);
    let alloc_bytes = ALLOC_BYTES.load(Ordering::Relaxed);
    let realloc_calls = REALLOC_CALLS.load(Ordering::Relaxed);
    let realloc_bytes = REALLOC_BYTES.load(Ordering::Relaxed);
    let peak_live_bytes = PEAK_LIVE_BYTES.load(Ordering::Relaxed);
    let rss_hwm_kib = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")
                    .map(str::trim)
                    .map(str::to_owned)
            })
        })
        .unwrap_or_else(|| "unavailable".to_owned());
    println!("mode:{mode}");
    println!("iterations:{iterations}");
    println!("diagnostic_ns:{elapsed_ns}");
    println!("alloc_count:{alloc_calls}");
    println!("alloc_bytes:{alloc_bytes}");
    println!("realloc_count:{realloc_calls}");
    println!("realloc_bytes:{realloc_bytes}");
    println!("peak_live_bytes:{peak_live_bytes}");
    println!("rss_hwm_kib:{rss_hwm_kib}");
}
