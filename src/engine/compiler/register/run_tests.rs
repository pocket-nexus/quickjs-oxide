//! M3 and M4 acceptance: programs lowered from the compiler's stack code and
//! run by the new core's loop.

use std::path::Path;

use oxide_core::Runtime;
use oxide_core::bytecode::Opcode;
use oxide_core::heap::gc::HeapLimits;

use super::lower_script;
use crate::engine::compiler::compile_unlinked_script;

/// Runs `source` on the new core with `print` captured; an uncaught
/// exception appends `uncaught: <ToString>`.
pub(super) fn run_new_core(source: &str) -> String {
    run_with(source, |_| {}).0
}

/// [`run_new_core`] on a runtime `setup` adjusted first; also returns the
/// number of collections the run took.
fn run_with(source: &str, setup: impl FnOnce(&mut Runtime)) -> (String, u64) {
    let script = compile_unlinked_script(source).expect("compiles");
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    setup(&mut runtime);
    runtime.install_print().unwrap();
    runtime.capture_output();
    let lowered = lower_script(&mut runtime, &script).unwrap_or_else(|error| panic!("{error}"));
    let handle = runtime.hold(oxide_core::Value::object(lowered.function));
    let result = runtime.run_script(lowered.function);
    runtime.release(handle);
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
    assert_eq!(run_new_core("print(1 + 2)"), "3\n");
    assert_eq!(
        run_new_core("function f(a, b) { return a * b + 1; } print(f(6, 7), 'x')"),
        "43 x\n"
    );
}

/// Runs `source` on the current engine with a JavaScript `print` that
/// collects its lines, reporting an uncaught exception the same way.
fn run_current_core(source: &str) -> String {
    use crate::engine::api::Value;
    use crate::engine::test_support::test_context;

    let (runtime, mut context) = test_context();
    let _ = context
        .eval(
            "var __out = []; function print() { var line = ''; \
             for (var i = 0; i < arguments.length; i++) { \
             if (i) line += ' '; line += String(arguments[i]); } __out.push(line); }",
        )
        .unwrap();
    if context.eval(source).is_err() {
        let exception = context
            .take_exception()
            .unwrap()
            .unwrap_or(Value::Undefined);
        let Value::Object(report) = context
            .eval("(function (e) { __out.push('uncaught: ' + String(e)); })")
            .unwrap()
        else {
            panic!("report is not a function");
        };
        let report = runtime.as_callable(&report).unwrap().unwrap();
        let _ = context
            .call(&report, Value::Undefined, &[exception])
            .unwrap();
    }
    let Value::String(output) = context
        .eval("__out.length ? __out.join('\\n') + '\\n' : ''")
        .unwrap()
    else {
        panic!("output is not a string");
    };
    output.to_utf8_lossy()
}

fn corpus() -> Vec<(String, String)> {
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
            (name, std::fs::read_to_string(&path).unwrap())
        })
        .collect()
}

/// Every corpus program prints the same on both cores.
#[test]
fn corpus_matches_the_current_core() {
    let mut failures = Vec::new();
    for (name, source) in corpus() {
        let expected = run_current_core(&source);
        let actual = run_new_core(&source);
        if expected != actual {
            failures.push(format!(
                "==== {name}\n-- current core\n{expected}-- new core\n{actual}"
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
    let iterations = if cfg!(feature = "new-core-gc-stress") {
        3
    } else {
        20000
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
        runtime.heap_mut().collect();
    });
    assert_eq!(output, "200\n");
    assert!(collections > 5, "only {collections} collections");
    assert!(runtime_depth_after_run_is_zero());
}

fn runtime_depth_after_run_is_zero() -> bool {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let script = compile_unlinked_script("function f() { return 1; } f();").unwrap();
    let lowered = lower_script(&mut runtime, &script).unwrap();
    runtime.run_script(lowered.function).unwrap();
    runtime.vm_stack().depth() == 0
}

#[cfg(feature = "new-core-gc-stress")]
#[test]
fn stress_mode_collects_at_every_allocation() {
    let (output, collections) = run_with("var o = {}; o.a = [1, 2]; print('x' + o.a);", |_| {});
    assert_eq!(output, "x1,2\n");
    assert!(collections >= 4, "only {collections} collections");
}

/// The fixed probes of section 9, at a smoke-test iteration count, print the
/// same on both cores.
#[test]
fn fixed_probes_match_the_current_core() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/performance/probes/fixed");
    let mut failures = Vec::new();
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
        let (expected, actual) = (run_current_core(&source), run_new_core(&source));
        if expected != actual {
            failures.push(format!("{}: {expected:?} != {actual:?}", path.display()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Opcodes no compiled program lowers to: the wide shuttles (no function
/// needs more than 256 registers yet), stack-code forms the current compiler
/// never emits (tail calls, `return_undef`, the fused `typeof`/nullish tests,
/// a field-form `delete`), and `JUndefinedOrNull`, which fuses only the
/// for-of iterator-close pattern. `core/tests/vm.rs` runs each of them from
/// built units.
const NOT_EMITTED: &[Opcode] = &[
    Opcode::Nop,
    Opcode::LoadWide,
    Opcode::StoreWide,
    Opcode::TailCall,
    Opcode::TailCallMethod,
    Opcode::ReturnUndefined,
    Opcode::IsUndefined,
    Opcode::IsNull,
    Opcode::TypeOfIsUndefined,
    Opcode::TypeOfIsFunction,
    Opcode::DeleteField,
    Opcode::JUndefinedOrNull,
];

/// The corpus and probes lower to every opcode the lowering can emit, so the
/// two-core comparison runs each of them at least once.
#[test]
fn corpus_covers_every_emitted_opcode() {
    let mut seen = std::collections::HashSet::new();
    for (_, source) in corpus() {
        let script = compile_unlinked_script(&source).unwrap();
        let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
        let lowered = lower_script(&mut runtime, &script).unwrap();
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
}

/// Measurement hook for `docs/performance/probes/new-core/phase2.py`: runs
/// the file `PROBE_FILE` on the core `PROBE_CORE` (`new` or `current`) and
/// prints its output. Ignored in ordinary runs.
#[test]
#[ignore = "measurement hook"]
fn run_probe_file() {
    let path = std::env::var("PROBE_FILE").expect("PROBE_FILE");
    let source = std::fs::read_to_string(path).unwrap();
    let output = match std::env::var("PROBE_CORE").as_deref() {
        Ok("current") => run_current_core(&source),
        _ => run_new_core(&source),
    };
    print!("{output}");
}

/// Measurement hook for M6 (section 11): lowers every function of every
/// `.js` file in `CENSUS_DIR` on its own and prints, per file, the
/// functions lowered, every reason a function was declined, and the
/// functions whose register count (`1 + params + locals + max depth`)
/// exceeds the 256-register window. Ignored in ordinary runs.
#[test]
#[ignore = "measurement hook"]
fn lowering_census() {
    use oxide_core::bytecode::CodeBuilder;
    use oxide_core::bytecode::builder::UnitLayout;

    use super::{LowerError, lower_function};
    use crate::engine::code::function::UnlinkedFunction;

    fn walk<'a>(
        function: &'a UnlinkedFunction,
        parent: Option<u16>,
        out: &mut Vec<(&'a UnlinkedFunction, Option<u16>)>,
    ) {
        out.push((function, parent));
        for constant in function.constants() {
            if let Some(child) = constant.as_child() {
                walk(child, Some(function.metadata().argument_count), out);
            }
        }
    }

    let directory = std::env::var("CENSUS_DIR").expect("CENSUS_DIR");
    let mut files: Vec<_> = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "js"))
        .collect();
    files.sort();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let source = std::fs::read_to_string(&path).unwrap();
        let script = match compile_unlinked_script(&source) {
            Ok(script) => script,
            Err(error) => {
                println!("{name}\tcompile error: {error}");
                continue;
            }
        };
        let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
        let mut builder = CodeBuilder::new();
        builder.op(Opcode::ReturnUndefined);
        let placeholder = runtime
            .publish(builder.finish(UnitLayout {
                register_count: 1,
                ..UnitLayout::default()
            }))
            .unwrap();
        let mut functions = Vec::new();
        walk(&script, None, &mut functions);
        let (mut lowered, mut reasons, mut wide) =
            (0, std::collections::BTreeMap::new(), Vec::new());
        for (function, parent) in &functions {
            let metadata = function.metadata();
            let depth =
                crate::engine::code::bytecode::compute_stack_size(function.code(), u16::MAX)
                    .map_or(0, |size| u32::from(size.max_stack));
            let registers =
                1 + u32::from(metadata.argument_count) + u32::from(metadata.local_count) + depth;
            if registers > 256 {
                let label = function
                    .func_name()
                    .map(|name| name.to_utf8_lossy())
                    .filter(|name| !name.is_empty())
                    .unwrap_or_else(|| "<anonymous>".into());
                wide.push(format!("{label} ({registers})"));
            }
            let children = function
                .constants()
                .iter()
                .filter(|constant| constant.as_child().is_some())
                .map(|_| placeholder)
                .collect();
            match lower_function(&mut runtime, function, *parent, children) {
                Ok(_) => lowered += 1,
                Err(LowerError::Unsupported(construct)) => {
                    *reasons.entry(construct).or_insert(0) += 1;
                }
                Err(error) => {
                    *reasons.entry(format!("error: {error}")).or_insert(0) += 1;
                }
            }
        }
        let reasons: Vec<String> = reasons
            .into_iter()
            .map(|(reason, count)| format!("{reason} ×{count}"))
            .collect();
        println!(
            "{name}\t{lowered}/{}\t{}\t{}",
            functions.len(),
            reasons.join(", "),
            wide.join(", ")
        );
    }
}
