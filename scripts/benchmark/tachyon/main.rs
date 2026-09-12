//! Minimal benchmark host for the pinned Tachyon VM; no engine changes.
use std::{
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use tachyon_compiler::{
    CompileOptions, Compiler, MediaType, SourceId, SourceMode, SourceName, SourceText,
};
use tachyon_gc::HeapLimit;
use tachyon_vm::{
    AtomHashSeed, AtomTableConfig, ExecutionBudget, HostProviderError, HostProviders, Isolate,
    IsolateConfig, RealmLimits, RunOutcome, StackLimits, WallClockProvider,
};
struct Clock;
impl WallClockProvider for Clock {
    fn unix_time_milliseconds(&mut self) -> Result<i64, HostProviderError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|x| x.as_millis() as i64)
            .map_err(|_| HostProviderError::Unavailable)
    }
}
fn run() -> Result<(), String> {
    let path = std::env::args()
        .nth(1)
        .ok_or("usage: tachyon-bench script.js")?;
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    // Load shell globals separately so the workload's directive prologue stays intact.
    let prelude = "var __qxo_output = ''; function print() { for (var i=0;i<arguments.length;i++) { if(i) __qxo_output += ' '; __qxo_output += String(arguments[i]); } __qxo_output += '\\n'; } var console = { log: print };";
    let options = CompileOptions {
        source_mode: SourceMode::Script,
        direct_eval: false,
    };
    let compile = |id, name: String, text: String| {
        Compiler
            .compile(
                SourceText::new(
                    SourceId::new(id),
                    SourceName::new(name),
                    MediaType::JavaScript,
                    Arc::<str>::from(text),
                ),
                options,
            )
            .map_err(|e| format!("compile: {e:?}"))
    };
    let shell = compile(0, "qxo-shell.js".into(), prelude.into())?;
    let module = compile(1, path, format!("{source}\n;__qxo_output;"))?;
    let config = IsolateConfig::new(
        AtomTableConfig::new(
            1_048_576,
            64 * 1024 * 1024,
            AtomHashSeed::new(7640891576956012809, 4354685564936845355),
        ),
        HeapLimit::new(2 * 1024 * 1024 * 1024),
        StackLimits::new(16384, 4 * 1024 * 1024),
        RealmLimits::new(1024, 65536),
    );
    let mut isolate =
        Isolate::new_with_host_providers(config, HostProviders::new().with_wall_clock(Clock))
            .map_err(|e| format!("isolate: {e:?}"))?;
    match isolate
        .execute(
            &shell,
            ExecutionBudget {
                fuel: u64::MAX,
                quantum: u32::MAX,
            },
        )
        .map_err(|e| format!("shell: {e:?}"))?
    {
        RunOutcome::Completed(_) => (),
        other => return Err(format!("shell outcome: {other:?}")),
    }
    let outcome = isolate
        .execute(
            &module,
            ExecutionBudget {
                fuel: u64::MAX,
                quantum: u32::MAX,
            },
        )
        .map_err(|e| format!("execute: {e:?}"))?;
    match outcome {
        RunOutcome::Completed(value) => {
            let units = isolate
                .string_value_to_utf16(value)
                .map_err(|e| format!("output: {e:?}"))?;
            print!("{}", String::from_utf16(&units).map_err(|e| e.to_string())?);
            Ok(())
        }
        RunOutcome::Thrown(value) => Err(format!(
            "thrown: {value:?}; native kind: {:?}",
            isolate.native_error_kind(value)
        )),
        other => Err(format!("outcome: {other:?}")),
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("tachyon-bench: {error}");
        std::process::exit(1);
    }
}
