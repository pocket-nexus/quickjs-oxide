//! Process-backed host services for the embedding API.

use oxide_core::host::{Monotonic, RejectionEvent, Stream};

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
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        return std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |elapsed| elapsed.as_nanos() as u64);
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        return 1;
    }

    /// `HostPromiseRejectionTracker(promise, operation)`. The promise is a
    /// value of the runtime; while rejection tracking is on
    /// ([`Runtime::set_track_rejections`](super::Runtime::set_track_rejections))
    /// the runtime also keeps every unhandled rejected promise alive in
    /// [`Runtime::unhandled_rejections`](super::Runtime::unhandled_rejections).
    /// The default ignores the event.
    fn promise_rejection(&mut self, event: RejectionEvent, promise: oxide_core::Value) {
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

    /// The independent monotonic nanosecond clock used to time collection.
    /// Targets without the process clock report zero-length pauses.
    fn monotonic(&mut self) -> Monotonic {
        #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
        return Monotonic::new(collection_now_ns);
        #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
        return Monotonic::NONE;
    }

    /// Milliseconds since the first process-wide performance clock read.
    /// Collection timing does not initialize this clock's origin.
    fn performance_now_ms(&mut self) -> f64 {
        use std::sync::OnceLock;
        use std::time::Instant;
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
    }

    /// Suspends the current thread for a positive number of milliseconds.
    fn sleep_ms(&mut self, milliseconds: f64) {
        if milliseconds > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(milliseconds / 1000.0));
        }
    }

    /// Adapts these services for a raw core runtime without allocating.
    fn into_core(self) -> impl oxide_core::host::Host
    where
        Self: Sized,
    {
        Hosted(self)
    }
}

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
fn collection_now_ns() -> u64 {
    use std::sync::LazyLock;
    use std::time::Instant;
    static START: LazyLock<Instant> = LazyLock::new(Instant::now);
    START.elapsed().as_nanos() as u64
}

/// The default host: the process's streams, the system clock, UTC.
#[derive(Debug, Default)]
pub struct SystemHost;

impl Host for SystemHost {}

/// Keeps the std defaults at the embedding boundary without another trait object.
pub(super) struct Hosted<H>(pub H);

impl<H: Host> oxide_core::host::Host for Hosted<H> {
    fn write(&mut self, stream: Stream, text: &[u8]) {
        self.0.write(stream, text);
    }

    fn now_ms(&mut self) -> f64 {
        self.0.now_ms()
    }

    fn local_tz_offset_minutes(&mut self, utc_ms: f64) -> i32 {
        self.0.local_tz_offset_minutes(utc_ms)
    }

    fn random_seed(&mut self) -> u64 {
        self.0.random_seed()
    }

    fn promise_rejection(&mut self, event: RejectionEvent, promise: oxide_core::Value) {
        self.0.promise_rejection(event, promise);
    }

    fn read_file(&mut self, path: &str) -> Option<String> {
        self.0.read_file(path)
    }

    fn interrupt(&mut self) -> bool {
        self.0.interrupt()
    }

    fn monotonic(&mut self) -> Monotonic {
        self.0.monotonic()
    }

    fn performance_now_ms(&mut self) -> f64 {
        self.0.performance_now_ms()
    }

    fn sleep_ms(&mut self, milliseconds: f64) {
        self.0.sleep_ms(milliseconds);
    }
}
