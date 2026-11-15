//! The host boundary: what the core asks of its embedder. `quickjs-oxide-core` reaches
//! the platform only through this trait, so it stays dependency-free and a port
//! supplies its own implementation.

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
    /// `JS_ToCStringLen` writes it. The default writes to the process's
    /// streams.
    fn write(&mut self, stream: Stream, text: &[u8]) {
        use std::io::Write;
        match stream {
            Stream::Stdout => {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(text);
                let _ = out.flush();
            }
            Stream::Stderr => {
                let mut out = std::io::stderr().lock();
                let _ = out.write_all(text);
                let _ = out.flush();
            }
        }
    }

    /// Milliseconds since the Unix epoch (`Date.now()`).
    fn now_ms(&mut self) -> f64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_millis() as f64)
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
    /// (`qjs --std`). The default reads the process's file system.
    fn read_file(&mut self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    /// The monotonic clock the collector times its pauses with
    /// ([collector statistics](crate::heap::gc::GcStats)). The default has no
    /// clock, so pauses read zero — as on a target without one.
    fn monotonic(&mut self) -> Monotonic {
        Monotonic::NONE
    }

    /// Polled about every thousand loop back-edges and calls while a
    /// script runs: `true` stops the script with an uncatchable
    /// `InternalError: interrupted`. The default never interrupts.
    fn interrupt(&mut self) -> bool {
        false
    }
}

/// The default host: the process's streams, the system clock, UTC.
#[derive(Debug, Default)]
pub struct SystemHost;

impl Host for SystemHost {}
