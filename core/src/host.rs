//! The host boundary (phase 4, section 6.3): what the core asks of its
//! embedder. `oxide-core` reaches the platform only through this trait, so
//! it stays dependency-free and a port (the PSP build of phase 5) supplies
//! its own implementation.

/// Which output stream `print` and the console write to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// The embedder's services. Every method has a default suitable for a
/// standalone runtime: the process's streams, the system clock and UTC.
pub trait Host {
    /// Writes host output (`print`). The default writes to the process's
    /// streams.
    fn write(&mut self, stream: Stream, text: &str) {
        use std::io::Write;
        match stream {
            Stream::Stdout => {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(text.as_bytes());
                let _ = out.flush();
            }
            Stream::Stderr => {
                let mut out = std::io::stderr().lock();
                let _ = out.write_all(text.as_bytes());
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
}

/// The default host: the process's streams, the system clock, UTC.
#[derive(Debug, Default)]
pub struct SystemHost;

impl Host for SystemHost {}
