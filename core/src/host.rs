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

/// A monotonic reading in nanoseconds, handed over as a plain function.
///
/// The collector times its own pauses from paths that have no host in hand —
/// an allocation that trips the pressure trigger is inside `Heap`. So the
/// clock arrives the way QuickJS takes `JSMallocFunctions`: a function
/// pointer the embedder installs once, not a method the core calls. A port
/// with no clock leaves [`Monotonic::NONE`] in place and its pauses read zero.
#[derive(Clone, Copy, Debug)]
pub struct Monotonic(fn() -> u64);

impl Default for Monotonic {
    fn default() -> Monotonic {
        Monotonic::NONE
    }
}

impl Monotonic {
    /// No clock: every reading is zero.
    pub const NONE: Monotonic = Monotonic(no_clock);
    /// The embedder's clock: nanoseconds from any fixed point, never going
    /// backwards.
    pub const fn new(read: fn() -> u64) -> Monotonic {
        Monotonic(read)
    }

    /// A reading, in nanoseconds.
    #[inline]
    pub fn now_ns(self) -> u64 {
        (self.0)()
    }
}

const fn no_clock() -> u64 {
    0
}

/// The embedder's services. Every method has a default that needs no
/// operating system: no output, no clock, no files. A port that has them
/// overrides them, as `quickjs-oxide-host` does.
pub trait Host {
    /// Writes host output (`print`): UTF-8, with a lone surrogate of the
    /// JavaScript string encoded as is (WTF-8), as QuickJS's
    /// `JS_ToCStringLen` writes it. The default discards it: the core has no
    /// stream to write to. An embedder with a process writes to its standard
    /// streams, as `quickjs-oxide`'s and `quickjs-oxide-host`'s hosts do.
    fn write(&mut self, stream: Stream, text: &[u8]) {
        let _ = (stream, text);
    }

    /// Milliseconds since the Unix epoch (`Date.now()`). The default is the
    /// epoch itself: the core has no clock.
    fn now_ms(&mut self) -> f64 {
        0.0
    }

    /// UTC minus local time, in minutes, at `utc_ms`
    /// (`Date.prototype.getTimezoneOffset`'s sign). The default is UTC.
    fn local_tz_offset_minutes(&mut self, _utc_ms: f64) -> i32 {
        0
    }

    /// The initial state of `Math.random`, as QuickJS seeds from host time.
    /// The default is a constant: a clock-derived seed is the port's to
    /// supply, and `quickjs-oxide-host` seeds from the system clock.
    fn random_seed(&mut self) -> u64 {
        1
    }

    /// `HostPromiseRejectionTracker(promise, operation)`. The promise is a
    /// value of the runtime; while rejection tracking is on
    /// ([`Runtime::set_track_rejections`](crate::Runtime::set_track_rejections))
    /// the runtime also keeps every unhandled rejected promise alive in
    /// [`Runtime::unhandled_rejections`](crate::Runtime::unhandled_rejections).
    /// The default ignores the event.
    fn promise_rejection(&mut self, event: RejectionEvent, promise: crate::Value) {
        let _ = (event, promise);
    }

    /// The text of the file at `path`, for the CLI's `load` and `read`
    /// (`qjs --std`). The default finds no file system and returns `None`.
    fn read_file(&mut self, path: &str) -> Option<String> {
        let _ = path;
        None
    }

    /// The monotonic clock the collector times its pauses with
    /// ([collector statistics](crate::heap::gc::GcStats)). The default has no
    /// clock, so pauses read zero — as on a target without one.
    fn monotonic(&mut self) -> Monotonic {
        Monotonic::NONE
    }

    /// Blocks the calling thread for `milliseconds` (`$262.agent.sleep`). The
    /// default has no clock to wait on and returns at once: a target with one
    /// thread has nothing to yield to.
    fn sleep_ms(&mut self, _milliseconds: f64) {}

    /// Polled about every thousand loop back-edges and calls while a
    /// script runs: `true` stops the script with an uncatchable
    /// `InternalError: interrupted`. The default never interrupts.
    fn interrupt(&mut self) -> bool {
        false
    }
}

/// The host a runtime starts with, until the embedder installs its own: no
/// output, no clock, no files, a constant seed.
#[derive(Debug, Default)]
pub(crate) struct NoHost;

impl Host for NoHost {}
