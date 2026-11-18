//! The host boundary: what the core asks of its embedder. `quickjs-oxide-core` reaches
//! the platform only through this trait, so it stays dependency-free and a port
//! supplies its own implementation.

use alloc::string::String;

/// Which output stream `print` and the console write to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// What [`Host::promise_rejection`] reports (ECMAScript's
/// `HostPromiseRejectionTracker` operations).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejectionEvent {
    /// A promise was rejected with no handler attached (`"reject"`).
    Rejected,
    /// A handler was attached to a promise already rejected without one
    /// (`"handle"`).
    Handled,
}

/// A monotonic nanosecond reading available without borrowing the host.
///
/// Collection can start inside an allocation, where only the heap is in hand.
/// The host installs this capability once instead of being called from the
/// collector. A port without a clock leaves [`Monotonic::NONE`] installed.
#[derive(Clone, Copy, Debug)]
pub struct Monotonic(fn() -> u64);

impl Default for Monotonic {
    fn default() -> Self {
        Self::NONE
    }
}

impl Monotonic {
    pub const NONE: Self = Self(no_clock);

    pub const fn new(read: fn() -> u64) -> Self {
        Self(read)
    }

    pub fn read(self) -> u64 {
        (self.0)()
    }
}

const fn no_clock() -> u64 {
    0
}

/// The core's platform capabilities. Defaults require no operating system.
///
/// The std embedding API supplies process-backed defaults through its own
/// `Host` facade; a bare core runtime starts without those services.
pub trait Host {
    /// Writes WTF-8 output to the requested stream. Without a stream, output
    /// is discarded.
    fn write(&mut self, stream: Stream, text: &[u8]) {
        let _ = (stream, text);
    }

    /// Milliseconds since the Unix epoch. Without a clock, the epoch.
    fn now_ms(&mut self) -> f64 {
        0.0
    }

    /// UTC minus local time, in minutes, at `utc_ms`. The default is UTC.
    fn local_tz_offset_minutes(&mut self, _utc_ms: f64) -> i32 {
        0
    }

    /// The initial `Math.random` state; a port may supply its own entropy.
    fn random_seed(&mut self) -> u64 {
        1
    }

    /// ECMAScript's `HostPromiseRejectionTracker` notification.
    fn promise_rejection(&mut self, event: RejectionEvent, promise: crate::Value) {
        let _ = (event, promise);
    }

    /// Reads a text file. Without a file service, no file is found.
    fn read_file(&mut self, path: &str) -> Option<String> {
        let _ = path;
        None
    }

    /// Polled at execution checkpoints; `true` interrupts the script.
    fn interrupt(&mut self) -> bool {
        false
    }

    /// The independent nanosecond clock used to time collection pauses.
    fn monotonic(&mut self) -> Monotonic {
        Monotonic::NONE
    }

    /// The host's performance clock, in milliseconds. Its origin is the
    /// host's policy, independent of collection timing.
    fn performance_now_ms(&mut self) -> f64 {
        0.0
    }

    /// Suspends execution for the requested interval when a sleep service
    /// exists. A port without one has nothing to yield to.
    fn sleep_ms(&mut self, _milliseconds: f64) {}
}

#[derive(Debug, Default)]
pub(crate) struct NoHost;

impl Host for NoHost {}
