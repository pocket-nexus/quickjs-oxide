//! Collector measurements over the monotonic capability installed by a host.

use core::time::Duration;

use crate::host::Monotonic;

/// A started measurement.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Stopwatch {
    clock: Monotonic,
    start: u64,
}

impl Stopwatch {
    #[inline]
    pub(crate) fn start(clock: Monotonic) -> Self {
        Self {
            clock,
            start: clock.read(),
        }
    }

    #[inline]
    pub(crate) fn elapsed(self) -> Duration {
        Duration::from_nanos(self.clock.read().saturating_sub(self.start))
    }
}
