//! The process host: what the core has no way to reach.
//!
//! `quickjs-oxide-core` runs without an operating system, so its `Host` defaults
//! supply nothing — output is discarded, `Date.now()` is the epoch, no file is
//! read. This crate is a std crate and every embedder of it has a process, so
//! its default host is that process's: the standard streams, the system clock,
//! the file system and UTC. `quickjs-oxide-host` adds the host's own time-zone
//! rules on top.

use std::io::Write;
use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use oxide_core::host::{Host, Monotonic, Stream};

/// The process's streams, the system clock, the file system and UTC.
#[derive(Debug, Default)]
pub struct SystemHost;

impl Host for SystemHost {
    fn write(&mut self, stream: Stream, text: &[u8]) {
        match stream {
            Stream::Stdout => write_and_flush(&mut std::io::stdout().lock(), text),
            Stream::Stderr => write_and_flush(&mut std::io::stderr().lock(), text),
        }
    }

    fn now_ms(&mut self) -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |elapsed| elapsed.as_millis() as f64)
    }

    fn random_seed(&mut self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(1, |elapsed| elapsed.as_nanos() as u64)
    }

    fn read_file(&mut self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn monotonic(&mut self) -> Monotonic {
        Monotonic::new(monotonic_ns)
    }

    fn sleep_ms(&mut self, milliseconds: f64) {
        if milliseconds > 0.0 {
            std::thread::sleep(std::time::Duration::from_secs_f64(milliseconds / 1000.0));
        }
    }
}

/// A failed or closed stream is not the runtime's to report: QuickJS ignores
/// the return value of its own `fwrite` too.
fn write_and_flush(out: &mut dyn Write, text: &[u8]) {
    let _ = out.write_all(text);
    let _ = out.flush();
}

/// Nanoseconds from the first call, for the collector's pause statistics.
fn monotonic_ns() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_nanos() as u64
}
