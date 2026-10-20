//! The monotonic clock the collector times its pauses with. A target
//! without one (`wasm32-unknown-unknown`, the web build's) reports
//! zero-length pauses instead of panicking.

use std::time::Duration;

/// A started measurement.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stopwatch {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    start: std::time::Instant,
}

impl Stopwatch {
    #[inline]
    pub(crate) fn start() -> Stopwatch {
        Stopwatch {
            #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
            start: std::time::Instant::now(),
        }
    }

    #[inline]
    pub(crate) fn elapsed(self) -> Duration {
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        return self.start.elapsed();
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        return Duration::ZERO;
    }
}

/// A seed from the system clock's microseconds; a constant on a target
/// without one (its embedder's host seeds `Math.random` instead).
pub(crate) fn time_seed() -> u64 {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    return std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1, |elapsed| elapsed.as_nanos() as u64);
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    return 1;
}
