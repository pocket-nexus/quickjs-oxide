//! The new core's driver entry (`docs/performance/new-core-phase2.md`,
//! section 5): compile a script with the existing front end, lower every
//! function to register code and run it on `oxide-core` with `print` on the
//! global object. The `oxide-run` binary of the CLI crate is its only user;
//! it is not an embedding interface and changes with the new core.

use std::fmt;

use oxide_core::Runtime;
use oxide_core::heap::gc::HeapLimits;

use crate::engine::api::error::Error;
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::compiler::compile_unlinked_script_bytes_with_filename;
use crate::engine::compiler::register::{LowerError, lower_script};

/// Whether this build has the new core's stress collector
/// (`new-core-gc-stress`), which `--gc-stress` needs.
pub const GC_STRESS_AVAILABLE: bool = cfg!(feature = "new-core-gc-stress");

/// Whether this build counts inline-cache hits (`new-core-ic-stats`), which
/// `--ic-stats` needs.
pub const IC_STATS_AVAILABLE: bool = cfg!(feature = "new-core-ic-stats");

#[derive(Clone, Debug, Default)]
pub struct NewCoreOptions {
    /// Print the inline-cache report to stderr at exit (`--ic-stats`); needs
    /// [`IC_STATS_AVAILABLE`].
    pub ic_stats: bool,
    /// Print the collector's statistics to stderr at exit (`--gc-stats`):
    /// collections, total and maximum pause, pause percentiles, live bytes
    /// and the heap reserved.
    pub gc_stats: bool,
    /// Collect before every N-th allocating operation (`--gc-stress=N`);
    /// needs [`GC_STRESS_AVAILABLE`].
    pub gc_stress: Option<u32>,
    /// The heap cap in bytes; `None` for none.
    pub heap_cap: Option<usize>,
}

#[derive(Debug)]
pub enum NewCoreError {
    /// The front end rejected the source.
    Compile(Error),
    /// The lowering declined a construct outside the phase 2 surface.
    Unsupported(String),
    /// The lowering or the runtime failed internally.
    Internal(String),
    /// The script threw; the thrown value's string.
    Uncaught(String),
    /// `gc_stress` was asked of a build without the stress collector.
    StressUnavailable,
    /// `ic_stats` was asked of a build without the cache counters.
    IcStatsUnavailable,
}

impl fmt::Display for NewCoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NewCoreError::Compile(error) => write!(f, "{error}"),
            NewCoreError::Unsupported(construct) => write!(f, "unsupported: {construct}"),
            NewCoreError::Internal(message) => write!(f, "internal error: {message}"),
            NewCoreError::Uncaught(text) => f.write_str(text),
            NewCoreError::StressUnavailable => {
                f.write_str("--gc-stress needs a build with the new-core-gc-stress feature")
            }
            NewCoreError::IcStatsUnavailable => {
                f.write_str("--ic-stats needs a build with the new-core-ic-stats feature")
            }
        }
    }
}

impl std::error::Error for NewCoreError {}

impl From<LowerError> for NewCoreError {
    fn from(error: LowerError) -> Self {
        match error {
            LowerError::Unsupported(construct) => NewCoreError::Unsupported(construct),
            other => NewCoreError::Internal(other.to_string()),
        }
    }
}

fn runtime(options: &NewCoreOptions) -> Result<Runtime, NewCoreError> {
    let limits = HeapLimits {
        max_bytes: options.heap_cap.unwrap_or(usize::MAX),
    };
    let mut runtime =
        Runtime::new(limits).map_err(|error| NewCoreError::Internal(error.to_string()))?;
    configure_stress(&mut runtime, options.gc_stress)?;
    runtime.heap_mut().gc_mut().record_pauses = options.gc_stats;
    runtime
        .install_print()
        .map_err(|error| NewCoreError::Internal(error.to_string()))?;
    Ok(runtime)
}

/// A stress build collects at every allocation unless told otherwise; the
/// driver collects normally unless `--gc-stress` asks.
#[cfg(feature = "new-core-gc-stress")]
fn configure_stress(runtime: &mut Runtime, interval: Option<u32>) -> Result<(), NewCoreError> {
    let gc = runtime.heap_mut().gc_mut();
    gc.stress = interval.is_some_and(|interval| interval > 0);
    gc.stress_interval = interval.unwrap_or(1).max(1);
    Ok(())
}

#[cfg(not(feature = "new-core-gc-stress"))]
fn configure_stress(_: &mut Runtime, interval: Option<u32>) -> Result<(), NewCoreError> {
    match interval {
        Some(_) => Err(NewCoreError::StressUnavailable),
        None => Ok(()),
    }
}

fn lower(
    runtime: &mut Runtime,
    source: &[u8],
    filename: &str,
) -> Result<crate::engine::compiler::register::LoweredScript, NewCoreError> {
    let script = compile_unlinked_script_bytes_with_filename(source, filename, DebugInfoMode::Full)
        .map_err(NewCoreError::Compile)?;
    Ok(lower_script(runtime, &script)?)
}

/// Compiles, lowers and runs `source`; `print` writes to stdout.
pub fn run(source: &[u8], filename: &str, options: &NewCoreOptions) -> Result<(), NewCoreError> {
    if options.ic_stats && !IC_STATS_AVAILABLE {
        return Err(NewCoreError::IcStatsUnavailable);
    }
    let mut runtime = runtime(options)?;
    let lowered = lower(&mut runtime, source, filename)?;
    let function = oxide_core::Value::object(lowered.function);
    let handle = runtime.hold(function);
    let result = runtime.run_script(lowered.function);
    runtime.release(handle);
    if options.ic_stats
        && let Some(report) = runtime.ic_report(10)
    {
        eprint!("{report}");
    }
    if options.gc_stats {
        eprint!("{}", gc_report(&runtime));
    }
    match result {
        Ok(_) => Ok(()),
        Err(thrown) => {
            let handle = runtime.hold(thrown.0);
            let text = runtime
                .display(thrown.0)
                .unwrap_or_else(|_| "exception while printing the exception".into());
            runtime.release(handle);
            Err(NewCoreError::Uncaught(text))
        }
    }
}

/// The collector's statistics: Table D of the phase 3 plan.
fn gc_report(runtime: &Runtime) -> String {
    let gc = runtime.heap().gc();
    let stats = gc.stats;
    let mut pauses = gc.pauses.clone();
    pauses.sort();
    let percentile = |p: usize| {
        pauses
            .get((pauses.len() * p / 100).min(pauses.len().saturating_sub(1)))
            .map_or(0.0, |pause| pause.as_secs_f64() * 1e3)
    };
    format!(
        "gc: {} collections, total pause {:.1} ms, p50 {:.3} ms, p99 {:.3} ms, max {:.3} ms, \
         live {} KiB, reserved {} KiB\n",
        stats.collections,
        stats.total_pause.as_secs_f64() * 1e3,
        percentile(50),
        percentile(99),
        stats.max_pause.as_secs_f64() * 1e3,
        stats.live_bytes / 1024,
        runtime.heap().bytes() / 1024,
    )
}

/// The disassembly of every unit `source` lowers to, root first.
pub fn disassemble(source: &[u8], filename: &str) -> Result<String, NewCoreError> {
    let mut runtime = runtime(&NewCoreOptions::default())?;
    let lowered = lower(&mut runtime, source, filename)?;
    let mut text = String::new();
    for unit in runtime.code().tree(lowered.root) {
        text.push_str(&runtime.disassemble(unit));
        text.push('\n');
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_classified() {
        let options = NewCoreOptions::default();
        assert!(matches!(
            run(b"class C {}", "t.js", &options),
            Err(NewCoreError::Unsupported(construct)) if construct == "classes"
        ));
        assert!(matches!(
            run(b"var x = ;", "t.js", &options),
            Err(NewCoreError::Compile(_))
        ));
        let Err(NewCoreError::Uncaught(text)) = run(b"null.x", "t.js", &options) else {
            panic!("no uncaught error");
        };
        assert_eq!(text, "TypeError: cannot read property 'x' of null");
        let text = disassemble(b"function f(a) { return a + 1; }", "t.js").unwrap();
        assert!(text.contains("AddI"), "{text}");
    }
}
