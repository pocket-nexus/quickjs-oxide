//! Programs compiled by the front end and run by the core's loop.

use std::path::Path;

use oxide_core::Runtime;
use oxide_core::bytecode::Opcode;
use oxide_core::heap::gc::HeapLimits;

use super::compile::{LoweredScript, compile_script};
use crate::engine::api::core::compile::DebugInfoMode;
use crate::engine::ast::ProgramKind;
use crate::source::SourceText;

/// Compiles the script `source` into `runtime` and instantiates it.
pub(crate) fn lower_source(runtime: &mut Runtime, source: &str) -> LoweredScript {
    let text = SourceText::try_from_raw_bytes(source.as_bytes()).expect("valid source");
    compile_script(
        &mut runtime.compiler(),
        &text,
        "<input>",
        DebugInfoMode::Full,
        ProgramKind::Script,
    )
    .unwrap_or_else(|error| panic!("{error:?}"))
}

/// Runs `source` on the core with `print` captured; an uncaught
/// exception appends `uncaught: <ToString>`.
pub(crate) fn run_on_core(source: &str) -> String {
    run_with(source, |_| {}).0
}

/// [`run_on_core`] on a runtime `setup` adjusted first; also returns the
/// number of collections the run took.
fn run_with(source: &str, setup: impl FnOnce(&mut Runtime)) -> (String, u64) {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    setup(&mut runtime);
    // `eval` and the `Function` constructors compile through the API's host.
    runtime.set_compiler(crate::engine::api::core::compile::compile_for_core);
    runtime.install_print().unwrap();
    runtime.capture_output();
    let lowered = lower_source(&mut runtime, source);
    let handle = runtime.hold(oxide_core::Value::object(lowered.function));
    let mut result = runtime.run_script(lowered.function);
    runtime.release(handle);
    if result.is_ok() {
        result = runtime.run_jobs().map(|()| oxide_core::Value::UNDEFINED);
    }
    let mut output = runtime.take_output();
    if let Err(thrown) = result {
        let handle = runtime.hold(thrown.0);
        let text = runtime
            .display(thrown.0)
            .unwrap_or_else(|_| "<unprintable>".into());
        runtime.release(handle);
        output.push_str(&format!("uncaught: {text}\n"));
    }
    (output, runtime.heap().gc().stats.collections)
}

#[test]
fn smoke() {
    assert_eq!(run_on_core("print(1 + 2)"), "3\n");
    assert_eq!(
        run_on_core("function f(a, b) { return a * b + 1; } print(f(6, 7), 'x')"),
        "43 x\n"
    );
}

/// The corpus's programs with the output each must print (`NAME.out` beside
/// `NAME.js`); `builtins_number` prints the fewest digits of a power of two,
/// as the specification says (QUICKJS-SPEC-036).
pub(crate) fn corpus() -> Vec<(String, String)> {
    corpus_with_outputs()
        .into_iter()
        .map(|(name, source, _)| (name, source))
        .collect()
}

fn corpus_with_outputs() -> Vec<(String, String, String)> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("core/tests/corpus");
    let mut files: Vec<_> = std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "js"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|path| {
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let output = std::fs::read_to_string(path.with_extension("out")).unwrap();
            (name, std::fs::read_to_string(&path).unwrap(), output)
        })
        .collect()
}

/// Every corpus program prints its recorded output.
#[test]
fn corpus_prints_its_recorded_outputs() {
    let mut failures = Vec::new();
    for (name, source, expected) in corpus_with_outputs() {
        let actual = run_on_core(&source);
        if expected != actual {
            failures.push(format!(
                "==== {name}\n-- expected\n{expected}-- actual\n{actual}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A collection with 200 frames live keeps every frame's registers: each
/// frame checks its own object, string and closure after the innermost one
/// has allocated enough to collect many times.
#[test]
fn collection_under_200_frames_preserves_registers() {
    // Under the stress collector every allocation collects already.
    let iterations = if cfg!(feature = "gc-stress") {
        3
    } else {
        80000
    };
    let source = "function deep(n) { \
                    var o = { v: n }, s = 'k' + n, f = () => n * 2, a = [n, o]; \
                    if (n > 0) { \
                      var r = deep(n - 1); \
                      if (o.v !== n || s !== 'k' + n || f() !== n * 2 || a[1] !== o) \
                        throw new Error('frame ' + n + ' lost a register'); \
                      return r + 1; \
                    } \
                    var junk = []; \
                    for (var i = 0; i < ITERATIONS; i++) junk = [junk, { i: i }, 'x' + i]; \
                    return 0; \
                  } \
                  print(deep(200));"
        .replace("ITERATIONS", &iterations.to_string());
    let (output, collections) = run_with(&source, |runtime| {
        let gc = runtime.heap_mut().gc_mut();
        gc.min_heap = 64 << 10;
        // `junk` stays live, so the trigger grows with it; a small factor
        // keeps collections frequent above the startup heap.
        gc.growth_factor = 1.25;
        runtime.heap_mut().collect();
    });
    assert_eq!(output, "200\n");
    assert!(collections > 5, "only {collections} collections");
    assert!(runtime_depth_after_run_is_zero());
}

fn runtime_depth_after_run_is_zero() -> bool {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let lowered = lower_source(&mut runtime, "function f() { return 1; } f();");
    runtime.run_script(lowered.function).unwrap();
    runtime.vm_stack().depth() == 0
}

#[cfg(feature = "gc-stress")]
#[test]
fn stress_mode_collects_at_every_allocation() {
    let (output, collections) = run_with("var o = {}; o.a = [1, 2]; print('x' + o.a);", |_| {});
    assert_eq!(output, "x1,2\n");
    assert!(collections >= 4, "only {collections} collections");
}

/// What each fixed probe (`docs/performance/probes/fixed`) prints with
/// `print(work(37…`.
const FIXED_PROBE_OUTPUTS: &[(&str, &str)] = &[
    ("argument_move", "36\n"),
    ("array_read", "91\n"),
    ("array_write", "138\n"),
    ("bigint_256", "true\n"),
    ("bigint_32", "true\n"),
    ("bigint_64", "true\n"),
    ("call0", "37\n"),
    ("empty_loop", "37\n"),
    ("local_move", "72\n"),
    ("local_read", "41\n"),
    ("local_read_with_setup", "41\n"),
    ("number_owner_fallback", "37\n"),
    ("numeric_coercion_loop", "37\n"),
    ("numeric_local_loop", "37\n"),
    ("object_move", "2\n"),
    ("prop_read", "111\n"),
    ("prop_write", "36\n"),
    ("string_bridge", "37\n"),
    ("tdz", "37\n"),
    ("type_error", "37\n"),
];

/// The fixed probes, at a smoke-test iteration count, print their recorded
/// outputs.
#[test]
fn fixed_probes_print_their_recorded_outputs() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/performance/probes/fixed");
    let mut failures = Vec::new();
    let mut seen = 0;
    for entry in std::fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|extension| extension != "js") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        // `print(work(N` with a small N.
        let call = source.rfind("print(work(").expect("a work call") + "print(work(".len();
        let end = call + source[call..].find([',', ')']).unwrap();
        let source = format!("{}37{}", &source[..call], &source[end..]);
        let name = path.file_stem().unwrap().to_string_lossy();
        let expected = FIXED_PROBE_OUTPUTS
            .iter()
            .find(|(probe, _)| *probe == name)
            .unwrap_or_else(|| panic!("{name} has no recorded output"))
            .1;
        seen += 1;
        let actual = run_on_core(&source);
        if expected != actual {
            failures.push(format!("{}: {expected:?} != {actual:?}", path.display()));
        }
    }
    assert_eq!(seen, FIXED_PROBE_OUTPUTS.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Opcodes no compiled program lowers to: the wide shuttles (locals and
/// temporaries past the window live in spill slots), forms the lowering
/// never selects (tail calls, the fused `typeof`/nullish tests, a field-form
/// `delete`, a standalone nullish test, which always feeds a branch and
/// fuses into `JUndefinedOrNull`), and opcodes that remain for built units.
/// `core/tests/vm.rs` runs each of them from built units.
const NOT_EMITTED: &[Opcode] = &[
    Opcode::Nop,
    Opcode::LoadWide,
    Opcode::StoreWide,
    Opcode::TailCall,
    Opcode::TailCallMethod,
    Opcode::IsUndefined,
    Opcode::IsNull,
    Opcode::IsUndefinedOrNull,
    Opcode::TypeOfIsUndefined,
    Opcode::TypeOfIsFunction,
    Opcode::DeleteField,
    // A direct `eval` call lowers as a call of `eval`, which runs as
    // indirect eval.
    Opcode::ThrowInternal,
    // `new.target` is a register.
    Opcode::LoadNewTarget,
    // Accessors in literals lower through `DefineMethod` with the getter
    // and setter flags.
    Opcode::DefineGetter,
    Opcode::DefineSetter,
    // A reference read through a prepared global reference, for a function
    // that also calls `eval` directly.
    Opcode::GetRef,
    // `import()` needs the module host, which the corpus's scripts run
    // without; Test262's dynamic-import tests run it.
    Opcode::ImportCall,
    // A cell's TDZ check loads the cell's value and checks it with `CheckInit`.
    Opcode::CheckCellInit,
];

/// The corpus and probes lower to every opcode the lowering can emit, so the
/// comparison with the recorded outputs runs each of them at least once.
#[test]
fn corpus_covers_every_emitted_opcode() {
    let mut seen = std::collections::HashSet::new();
    for (_, source) in corpus() {
        let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
        let lowered = lower_source(&mut runtime, &source);
        for unit in runtime.code().tree(lowered.root) {
            seen.extend(runtime.code().get(unit).code.iter().map(|insn| insn.op));
        }
    }
    let missing: Vec<&str> = Opcode::ALL
        .iter()
        .filter(|op| !seen.contains(*op) && !NOT_EMITTED.contains(op))
        .map(|op| op.name())
        .collect();
    assert!(missing.is_empty(), "not covered: {missing:?}");
    let emitted: Vec<&str> = NOT_EMITTED
        .iter()
        .filter(|op| seen.contains(*op))
        .map(|op| op.name())
        .collect();
    assert!(emitted.is_empty(), "listed as not emitted: {emitted:?}");
}

/// Measurement hook for `scripts/benchmark/bench.py slopes instructions`:
/// runs the file `PROBE_FILE` and prints its output. Ignored in ordinary runs.
#[test]
#[ignore = "measurement hook"]
fn run_probe_file() {
    let path = std::env::var("PROBE_FILE").expect("PROBE_FILE");
    let source = std::fs::read_to_string(path).unwrap();
    print!("{}", run_on_core(&source));
}
