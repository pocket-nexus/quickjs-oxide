//! The new core's driver entry (`docs/performance/new-core-phase2.md`,
//! section 5): compile a script with the existing front end, lower every
//! function to register code and run it on `oxide-core` with `print` on the
//! global object. The `oxide-run` binary of the CLI crate is its only user;
//! it is not an embedding interface and changes with the new core.

use std::fmt;

use oxide_core::Runtime;
use oxide_core::heap::gc::HeapLimits;

pub(crate) use crate::engine::api::core::compile::{
    ModuleCompileError, compile_module, core_error_kind,
};
use crate::engine::api::core::compile::{compile_for_core, normalize_module_name};
use crate::engine::api::error::Error;
use crate::engine::code::debug::DebugInfoMode;
use crate::engine::compiler::compile_unlinked_script_bytes_with_filename;
use crate::engine::compiler::register::{LowerError, lower_script};
use oxide_core::vm::compile::{CompileError, Compiler};

#[cfg(feature = "test262-host")]
pub mod test262;

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
    /// Define the Test262 host object `$262` (`--test262`).
    pub test262: bool,
    /// Forbid `Atomics.wait` from blocking (`--cannot-block`, Test262's
    /// `CanBlockIsFalse`).
    pub cannot_block: bool,
    /// Run the last file as a module (`--module`), the others as scripts.
    pub module: bool,
    /// Define qjs's `load` and `read` (`--std`).
    pub std: bool,
    /// qjs's `scriptArgs`, when set: the script's path and arguments.
    pub script_args: Option<Vec<String>>,
    /// The clock and the local time zone (section 6.3); `None` is the system
    /// clock in UTC.
    pub services: Option<std::rc::Rc<dyn crate::engine::host::HostServices>>,
}

/// The core's host hooks over the engine's host services.
struct ServicesHost(std::rc::Rc<dyn crate::engine::host::HostServices>);

impl oxide_core::host::Host for ServicesHost {
    fn now_ms(&mut self) -> f64 {
        self.0.now_millis() as f64
    }

    fn local_tz_offset_minutes(&mut self, utc_ms: f64) -> i32 {
        self.0.timezone_offset_minutes(utc_ms as i64)
    }
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
            LowerError::Syntax(message) => NewCoreError::Compile(Error::new(
                crate::engine::api::error::ErrorKind::Syntax,
                message,
            )),
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
    if let Some(services) = &options.services {
        runtime.set_host(Box::new(ServicesHost(services.clone())));
    }
    runtime.heap_mut().gc_mut().record_pauses = options.gc_stats;
    runtime.set_can_block(!options.cannot_block);
    runtime.set_compiler(compile_for_core);
    runtime
        .install_print()
        .map_err(|error| NewCoreError::Internal(error.to_string()))?;
    if options.std {
        runtime
            .install_std()
            .map_err(|error| NewCoreError::Internal(error.to_string()))?;
    }
    if let Some(args) = &options.script_args {
        runtime
            .install_script_args(args)
            .map_err(|error| NewCoreError::Internal(error.to_string()))?;
    }
    if options.test262 {
        runtime
            .install_test262_host()
            .map_err(|error| NewCoreError::Internal(error.to_string()))?;
        runtime.set_script_runner(run_agent_script);
    }
    runtime.set_module_host(oxide_core::vm::module::ModuleHost {
        normalize: normalize_module,
        load: load_module,
    });
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

thread_local! {
    /// The main file under `--test262`, for the loader's relative names.
    static MAIN_FILE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// QuickJS's `js_module_normalize_name`.
fn normalize_module(
    _: &mut Compiler<'_>,
    referrer: &str,
    specifier: &str,
    _: &[(String, String)],
) -> Result<String, CompileError> {
    Ok(normalize_module_name(referrer, specifier))
}

/// QuickJS's `js_module_loader`: the file named `name`, as JSON for a
/// `.json` name or a `type: "json"` attribute, else as a module.
fn load_module(
    compiler: &mut Compiler<'_>,
    name: &str,
    attributes: &[(String, String)],
) -> Result<oxide_core::vm::module::ModuleSource, CompileError> {
    let failure = |kind: oxide_core::ErrorKind, message: String| CompileError::new(kind, message);
    // As QuickJS's `run-test262` loader does, a name without a directory is
    // relative to the main file's.
    let relative;
    let name = match MAIN_FILE.with(|main| main.borrow().clone()) {
        Some(main) if !name.contains('/') => match main.rfind('/') {
            Some(slash) => {
                relative = format!("{}/{name}", &main[..slash]);
                relative.as_str()
            }
            None => name,
        },
        _ => name,
    };
    let source = std::fs::read(name).map_err(|_| {
        failure(
            oxide_core::ErrorKind::Reference,
            format!("could not load module filename '{name}'"),
        )
    })?;
    let json = attributes
        .iter()
        .any(|(key, value)| key == "type" && (value == "json" || value == "json5"));
    if json || name.ends_with(".json") {
        let text = String::from_utf8_lossy(&source);
        return Ok(oxide_core::vm::module::ModuleSource::Json(
            text.encode_utf16().collect(),
        ));
    }
    match compile_module(compiler, name, &source, DebugInfoMode::Full) {
        Ok((module, _)) => Ok(module),
        Err(ModuleCompileError::Front(error)) => {
            Err(failure(core_error_kind(&error), error.message().to_owned()))
        }
        Err(ModuleCompileError::Core(error)) => Err(error),
    }
}

/// Runs the module named `name` and its jobs; an uncaught rejection of its
/// evaluation is the failure.
fn run_module_file(runtime: &mut Runtime, name: &str) -> Result<(), NewCoreError> {
    let uncaught = |runtime: &mut Runtime, thrown: oxide_core::Value| {
        let handle = runtime.hold(thrown);
        let text = runtime
            .display(thrown)
            .unwrap_or_else(|_| "exception while printing the exception".into());
        runtime.release(handle);
        NewCoreError::Uncaught(text)
    };
    let promise = runtime
        .run_module(name)
        .map_err(|thrown| uncaught(runtime, thrown.0))?;
    let handle = runtime.hold(promise);
    let jobs = runtime.run_jobs();
    let state = runtime.promise_state(promise);
    runtime.release(handle);
    if let Err(thrown) = jobs {
        return Err(uncaught(runtime, thrown.0));
    }
    match state {
        Some((2, reason)) => Err(uncaught(runtime, reason)),
        _ => Ok(()),
    }
}

/// `$262.agent.start`'s runner: an agent's script and then its jobs.
fn run_agent_script(runtime: &mut Runtime, source: &str) -> Result<(), String> {
    let lowered =
        lower(runtime, source.as_bytes(), "<agent>").map_err(|error| error.to_string())?;
    let function = oxide_core::Value::object(lowered.function);
    let handle = runtime.hold(function);
    let mut result = runtime.run_script(lowered.function);
    runtime.release(handle);
    if result.is_ok() {
        result = runtime.run_jobs().map(|()| oxide_core::Value::UNDEFINED);
    }
    result.map(drop).map_err(|thrown| {
        runtime
            .display(thrown.0)
            .unwrap_or_else(|_| "exception while printing the exception".into())
    })
}

/// Compiles, lowers and runs `source`; `print` writes to stdout.
pub fn run(source: &[u8], filename: &str, options: &NewCoreOptions) -> Result<(), NewCoreError> {
    run_scripts(&[(source.to_vec(), filename.to_owned())], options)
}

/// Runs `scripts` in order as separate scripts of one realm, draining the
/// job queue after each (section 5.4); stops at the first that fails.
pub fn run_scripts(
    scripts: &[(Vec<u8>, String)],
    options: &NewCoreOptions,
) -> Result<(), NewCoreError> {
    if options.ic_stats && !IC_STATS_AVAILABLE {
        return Err(NewCoreError::IcStatsUnavailable);
    }
    let mut runtime = runtime(options)?;
    let mut outcome = Ok(());
    let (scripts_to_run, module) = match scripts.split_last() {
        Some((last, rest)) if options.module => (rest, Some(last)),
        _ => (scripts, None),
    };
    if options.test262 {
        MAIN_FILE.with(|main| *main.borrow_mut() = scripts.last().map(|(_, name)| name.clone()));
    }
    let scripts = scripts_to_run;
    for (source, filename) in scripts {
        let lowered = match lower(&mut runtime, source, filename) {
            Ok(lowered) => lowered,
            Err(error) => {
                outcome = Err(error);
                break;
            }
        };
        let function = oxide_core::Value::object(lowered.function);
        let handle = runtime.hold(function);
        let mut result = runtime.run_script(lowered.function);
        runtime.release(handle);
        if result.is_ok() {
            result = runtime.run_jobs().map(|()| oxide_core::Value::UNDEFINED);
        }
        if let Err(thrown) = result {
            let handle = runtime.hold(thrown.0);
            let text = runtime
                .display(thrown.0)
                .unwrap_or_else(|_| "exception while printing the exception".into());
            runtime.release(handle);
            outcome = Err(NewCoreError::Uncaught(text));
            break;
        }
    }
    if outcome.is_ok()
        && let Some((_, filename)) = module
    {
        outcome = run_module_file(&mut runtime, filename);
    }
    runtime.join_agents();
    if options.ic_stats
        && let Some(report) = runtime.ic_report(10)
    {
        eprint!("{report}");
    }
    if options.gc_stats {
        eprint!("{}", gc_report(&runtime));
    }
    outcome
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
        // Phase 0 dropped `with`: an early `SyntaxError`.
        assert!(matches!(
            run(b"with ({}) {}", "t.js", &options),
            Err(NewCoreError::Compile(error))
                if error.kind() == crate::engine::api::error::ErrorKind::Syntax
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
