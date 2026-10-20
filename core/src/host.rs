//! The host boundary: what the core asks of its embedder. `oxide-core` reaches
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

/// The embedder's services. Every method has a default suitable for a
/// standalone runtime: the process's streams, the system clock and UTC.
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

    /// The initial state of `Math.random`. The default derives it from the
    /// system clock, as QuickJS seeds from host time.
    fn random_seed(&mut self) -> u64 {
        crate::clock::time_seed()
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
