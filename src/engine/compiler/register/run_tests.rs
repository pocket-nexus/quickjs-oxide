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
pub(crate) fn run_new_core(source: &str) -> String {
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
    assert_eq!(run_new_core("print(1 + 2)"), "3\n");
    assert_eq!(
        run_new_core("function f(a, b) { return a * b + 1; } print(f(6, 7), 'x')"),
        "43 x\n"
    );
}

/// The corpus's programs with the output each printed on the old core at
/// phase 6 M3 (`NAME.out` beside `NAME.js`), where both cores printed the same,
/// with M4b's returns to the specification (`builtins_number` prints the
/// fewest digits of a power of two, QUICKJS-SPEC-036).
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

/// Every corpus program prints what it printed on the old core, or the
/// specification's result where M4b returned to it.
#[test]
fn corpus_matches_the_old_core() {
    let mut failures = Vec::new();
    for (name, source, expected) in corpus_with_outputs() {
        let actual = run_new_core(&source);
        if expected != actual {
            failures.push(format!(
                "==== {name}\n-- old core\n{expected}-- new core\n{actual}"
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
        // keeps collections frequent above phase 4's larger startup heap.
        gc.growth_factor = 1.25;
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

#[cfg(feature = "gc-stress")]
#[test]
fn stress_mode_collects_at_every_allocation() {
    let (output, collections) = run_with("var o = {}; o.a = [1, 2]; print('x' + o.a);", |_| {});
    assert_eq!(output, "x1,2\n");
    assert!(collections >= 4, "only {collections} collections");
}

/// What each fixed probe of section 9 printed on the old core at phase 6 M3,
/// with `print(work(37…`.
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

/// The fixed probes, at a smoke-test iteration count, print what they printed
/// on the old core.
#[test]
fn fixed_probes_match_the_old_core() {
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
        let actual = run_new_core(&source);
        if expected != actual {
            failures.push(format!("{}: {expected:?} != {actual:?}", path.display()));
        }
    }
    assert_eq!(seen, FIXED_PROBE_OUTPUTS.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Opcodes no compiled program lowers to: the wide shuttles (locals and
/// temporaries past the window live in spill slots), stack-code forms the current compiler
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
    // Emitted only for a direct `eval` call, which the current core runs.
    Opcode::ThrowInternal,
    // `new.target` is a register from phase 4 (section 4.5); the opcode
    // remains for built units.
    Opcode::LoadNewTarget,
    // Class constructors and methods with `super`: the corpus gains them
    // with classes (M3).
    Opcode::CheckCtor,
    Opcode::LoadHomeObject,
    // Accessors in literals lower through `DefineMethod` with the getter
    // and setter flags; the opcodes remain for built units.
    Opcode::DefineGetter,
    Opcode::DefineSetter,
    // A reference read through a prepared global reference, emitted only
    // in a function that also calls `eval` directly (M7).
    Opcode::GetRef,
    // `import()` needs the module host, which the corpus's scripts run
    // without; Test262's dynamic-import tests run it (phase 4, section 5.6).
    Opcode::ImportCall,
];

/// The corpus and probes lower to every opcode the lowering can emit, so the
/// comparison with the old core's outputs runs each of them at least once.
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
/// the file `PROBE_FILE` and prints its output. Ignored in ordinary runs.
#[test]
#[ignore = "measurement hook"]
fn run_probe_file() {
    let path = std::env::var("PROBE_FILE").expect("PROBE_FILE");
    let source = std::fs::read_to_string(path).unwrap();
    print!("{}", run_new_core(&source));
}

/// Measurement hook for M6 (section 11): lowers every function of every
/// `.js` file in `CENSUS_DIR` on its own and prints, per file, the
/// functions lowered, every reason a function was declined, and the
/// functions whose register count (`1 + params + locals + max depth`)
/// exceeds the 256-register window. With `CENSUS_TEST262` (a Test262
/// checkout's `test` directory, phase 4 section 8) it walks the tree
/// instead and prints the totals: files, functions, and each decline with
/// its count. Ignored in ordinary runs.
#[test]
#[ignore = "measurement hook"]
fn lowering_census() {
    if let Ok(directory) = std::env::var("CENSUS_TEST262") {
        return test262_census(Path::new(&directory));
    }
    let directory = std::env::var("CENSUS_DIR").expect("CENSUS_DIR or CENSUS_TEST262");
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
        let census = census(&script);
        let reasons: Vec<String> = census
            .reasons
            .into_iter()
            .map(|(reason, count)| format!("{reason} ×{count}"))
            .collect();
        println!(
            "{name}\t{}/{}\t{}\t{}",
            census.lowered,
            census.functions,
            reasons.join(", "),
            census.wide.join(", ")
        );
    }
}

/// What the lowering declines in one compiled script or module.
struct Census {
    functions: usize,
    lowered: usize,
    reasons: std::collections::BTreeMap<String, usize>,
    wide: Vec<String>,
}

/// Lowers every function of `script` on its own (children stand in as a
/// placeholder unit).
fn census(script: &crate::engine::code::function::UnlinkedFunction) -> Census {
    use oxide_core::bytecode::CodeBuilder;
    use oxide_core::bytecode::builder::UnitLayout;

    use super::{LowerError, lower_function};
    use crate::engine::code::function::UnlinkedFunction;

    fn walk<'a>(
        function: &'a UnlinkedFunction,
        parent: Option<super::Layout>,
        out: &mut Vec<(&'a UnlinkedFunction, Option<super::Layout>)>,
    ) {
        out.push((function, parent));
        let layout = super::analysis::analyze(function.code())
            .ok()
            .and_then(|analysis| super::Layout::of(function, &analysis).ok());
        for constant in function.constants() {
            if let Some(child) = constant.as_child() {
                walk(child, layout, out);
            }
        }
    }

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
    walk(script, None, &mut functions);
    let mut census = Census {
        functions: functions.len(),
        lowered: 0,
        reasons: std::collections::BTreeMap::new(),
        wide: Vec::new(),
    };
    for (function, parent) in &functions {
        let metadata = function.metadata();
        let depth = crate::engine::code::bytecode::compute_stack_size(function.code(), u16::MAX)
            .map_or(0, |size| u32::from(size.max_stack));
        let registers =
            1 + u32::from(metadata.argument_count) + u32::from(metadata.local_count) + depth;
        if registers > 256 {
            let label = function
                .func_name()
                .map(|name| name.to_utf8_lossy())
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "<anonymous>".into());
            census.wide.push(format!("{label} ({registers})"));
        }
        let children = function
            .constants()
            .iter()
            .filter(|constant| constant.as_child().is_some())
            .map(|_| placeholder)
            .collect();
        match lower_function(&mut runtime.compiler(), function, *parent, children) {
            Ok(_) => census.lowered += 1,
            Err(LowerError::Unsupported(construct) | LowerError::Syntax(construct)) => {
                *census.reasons.entry(construct).or_insert(0) += 1;
            }
            Err(error) => {
                *census.reasons.entry(format!("error: {error}")).or_insert(0) += 1;
            }
        }
    }
    census
}

/// [`lowering_census`] over a Test262 tree: every test file (not the
/// fixtures), compiled as its metadata says (a module, strict-only), with
/// the totals.
fn test262_census(directory: &Path) {
    fn collect(directory: &Path, out: &mut Vec<std::path::PathBuf>) {
        let mut entries: Vec<_> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "js")
                && !path.to_string_lossy().contains("_FIXTURE")
            {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    collect(directory, &mut files);
    let (mut compiled, mut rejected) = (0, 0);
    let mut total = Census {
        functions: 0,
        lowered: 0,
        reasons: std::collections::BTreeMap::new(),
        wide: Vec::new(),
    };
    let mut declining_files = 0;
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap_or_default();
        let flags = source
            .split("---*/")
            .next()
            .and_then(|metadata| metadata.split("flags:").nth(1))
            .and_then(|flags| flags.lines().next())
            .unwrap_or("");
        let strict = flags.contains("onlyStrict");
        let census = if flags.contains("module") {
            let name =
                crate::engine::value::JsString::try_from_utf8(&path.to_string_lossy()).unwrap();
            crate::engine::compiler::compile_unlinked_module_bytes_with_name(
                source.as_bytes(),
                name,
                crate::engine::code::debug::DebugInfoMode::Full,
            )
            .ok()
            .map(|module| census(module.function()))
        } else if strict {
            compile_unlinked_script(&format!("\"use strict\";\n{source}"))
                .ok()
                .map(|script| census(&script))
        } else {
            compile_unlinked_script(&source)
                .ok()
                .map(|script| census(&script))
        };
        let Some(census) = census else {
            rejected += 1;
            continue;
        };
        compiled += 1;
        total.functions += census.functions;
        total.lowered += census.lowered;
        if !census.reasons.is_empty() {
            declining_files += 1;
        }
        for (reason, count) in census.reasons {
            *total.reasons.entry(reason).or_insert(0) += count;
        }
        total.wide.extend(census.wide);
    }
    println!(
        "test262: {} files, {compiled} compiled ({rejected} rejected by the front end), \
         {} of {} functions lowered, {declining_files} files with a decline, {} past 256 registers",
        files.len(),
        total.lowered,
        total.functions,
        total.wide.len()
    );
    for (reason, count) in total.reasons {
        println!("  {reason} ×{count}");
    }
}

/// Development hook: prints the stack code of every function of the script
/// in `DUMP_SOURCE`, the lowering's input. Ignored in ordinary runs.
#[test]
#[ignore = "development hook"]
fn dump_stack_code() {
    use crate::engine::code::function::UnlinkedFunction;

    fn dump(function: &UnlinkedFunction, depth: usize) {
        let metadata = function.metadata();
        println!(
            "{:indent$}function {:?}: args {}, locals {}",
            "",
            function.func_name().map(|name| name.to_utf8_lossy()),
            metadata.argument_count,
            metadata.local_count,
            indent = depth * 2
        );
        for (pc, instruction) in function.code().iter().enumerate() {
            println!(
                "{:indent$}  {pc:4}  {instruction:?}",
                "",
                indent = depth * 2
            );
        }
        for constant in function.constants() {
            if let Some(child) = constant.as_child() {
                dump(child, depth + 1);
            }
        }
    }

    let source = std::env::var("DUMP_SOURCE").expect("DUMP_SOURCE");
    dump(&compile_unlinked_script(&source).unwrap(), 0);
}
