//! M2 acceptance for the lowering: the symbolic-stack rules, captured
//! bindings, handlers, and the corpus. The loop that runs this code is M3/M4;
//! until then these tests check the code itself, and every lowered unit
//! passes the publication validator.

use std::path::Path;

use oxide_core::Runtime;
use oxide_core::bytecode::CodeUnitId;
use oxide_core::heap::gc::HeapLimits;

use super::{LowerError, lower_script};
use crate::engine::code::bytecode::compute_stack_size;
use crate::engine::code::function::UnlinkedFunction;
use crate::engine::compiler::compile_unlinked_script;

fn runtime() -> Runtime {
    Runtime::new(HeapLimits::default()).unwrap()
}

fn lower(source: &str) -> Result<(Runtime, CodeUnitId), LowerError> {
    let script = compile_unlinked_script(source).expect("compiles");
    let mut runtime = runtime();
    let lowered = lower_script(&mut runtime, &script)?;
    let heap = runtime.heap();
    let bytecode = heap.function_bytecode(lowered.function).unwrap();
    assert_eq!(heap.bytecode_unit(bytecode), lowered.root);
    Ok((runtime, lowered.root))
}

/// The disassembly of every unit, root first.
fn disassemble(source: &str) -> Vec<String> {
    let (runtime, root) = lower(source).unwrap_or_else(|error| panic!("{error}"));
    runtime
        .code()
        .tree(root)
        .into_iter()
        .map(|unit| runtime.disassemble(unit))
        .collect()
}

/// The instruction lines of the unit named `name`, without PCs.
fn function(source: &str, name: &str) -> Vec<String> {
    let units = disassemble(source);
    let unit = units
        .iter()
        .find(|text| text.starts_with(&format!("{name}:")))
        .unwrap_or_else(|| panic!("no unit {name} in {units:#?}"));
    unit.lines()
        .skip(1)
        .map(|line| {
            let line = line.trim_start();
            match line.split_once("  ") {
                Some((pc, rest)) if pc.chars().all(|c| c.is_ascii_digit()) => {
                    rest.split_whitespace().collect::<Vec<_>>().join(" ")
                }
                _ => line.to_owned(),
            }
        })
        .collect()
}

#[test]
fn locals_are_read_in_place_and_results_retarget_into_locals() {
    let code = function(
        "function work(n) { var sum = 0; while (n > 0) { sum = sum + 1; n = n - 1; } return sum; }",
        "work",
    );
    assert_eq!(
        code,
        [
            "LoadInt l0, 0",
            "JNgtI p0, 0, @5",
            "AddI l0, l0, 1",
            "SubI p0, p0, 1",
            "Jmp @1",
            "Return l0",
        ]
    );
}

#[test]
fn a_counted_loop_is_a_compare_jump_and_an_increment() {
    let code = function(
        "function work(n) { var sum = 0; for (var i = 0; i < n; i++) sum = sum + i; return sum; }",
        "work",
    );
    // Two dispatches per iteration besides the body: the fused compare-jump
    // and the backward jump; `i++` is one increment in place.
    assert_eq!(
        code,
        [
            "LoadInt l0, 0",
            "LoadInt l1, 0",
            "JNlt l1, p0, @6",
            "Add l0, l0, l1",
            "Inc l1, l1",
            "Jmp @2",
            "Return l0",
        ]
    );
}

#[test]
fn rule_two_reads_the_old_value_before_a_write() {
    let code = function("function f(x) { return x + (x = 2); }", "f");
    assert_eq!(
        code[..3],
        [
            "Mov t0, p0".to_owned(),
            "LoadInt p0, 2".to_owned(),
            "Add t0, t0, p0".to_owned(),
        ]
    );

    // As an expression, the old value of `x++` is live.
    let code = function("function g(x) { x = x++ + x; return x; }", "g");
    // The old x is copied before the increment writes it back.
    assert_eq!(
        code[..3],
        [
            "ToNumeric t0, p0".to_owned(),
            "Inc p0, t0".to_owned(),
            "Add p0, t0, p0".to_owned(),
        ]
    );
}

#[test]
fn lexical_reads_check_the_tdz_once_per_block() {
    let code = function("function f() { let a = 1; let b = a + a; return b; }", "f");
    let checks = code
        .iter()
        .filter(|line| line.starts_with("CheckInit"))
        .count();
    assert!(checks <= 2, "{code:#?}");
    assert!(code[0].starts_with("LoadEmpty"), "{code:#?}");
}

#[test]
fn per_iteration_bindings_get_fresh_cells() {
    let source = "function make() { var fns = []; \
                  for (let i = 0; i < 3; i++) fns.push(() => i); return fns; }";
    let make = function(source, "make");
    assert!(
        make.iter().any(|line| line.starts_with("NewCellEmpty")),
        "{make:#?}"
    );
    assert!(
        make.iter()
            .filter(|line| line.starts_with("CloseCell"))
            .count()
            >= 2,
        "{make:#?}"
    );
    assert!(
        make.iter().any(|line| line.starts_with("MakeClosure")),
        "{make:#?}"
    );
    let units = disassemble(source);
    let arrow = units.last().unwrap();
    assert!(arrow.contains("LoadCaptured"), "{arrow}");
    assert!(arrow.contains("capture c0 <- parent 2"), "{arrow}");
    assert_eq!(
        arrow.matches("LoadCaptured").count(),
        1,
        "the TDZ check reads the loaded value: {arrow}"
    );
}

#[test]
fn captured_parameters_are_boxed_and_outer_captures_relay() {
    let source = "function outer(a) { var b = 1; \
                  return function() { return function() { return a + b; }; }; }";
    let outer = function(source, "outer");
    assert_eq!(outer[0], "NewCell p0, p0");
    assert_eq!(outer[1], "NewCell l0, l0");
    assert!(outer.contains(&"StoreCell l0, t0".to_owned()), "{outer:#?}");
    let units = disassemble(source);
    let innermost = units.last().unwrap();
    assert!(innermost.contains("capture c0 <- parent c0"), "{innermost}");
    assert!(innermost.contains("capture c1 <- parent c1"), "{innermost}");
}

#[test]
fn calls_overlap_the_callee_window() {
    let code = function(
        "function unit() { return 1; } \
         function work(n) { var sum = 0; for (var i = 0; i < n; i++) sum = sum + unit(); return sum; }",
        "work",
    );
    // `sum` stays in its register at depth 0, so the callee lands in t1.
    assert!(
        code.contains(&"GetGlobal t1, .unit, ic0".to_owned()),
        "{code:#?}"
    );
    assert!(code.contains(&"Call t1, t1, 0".to_owned()), "{code:#?}");
    assert!(code.contains(&"Add l0, l0, t1".to_owned()), "{code:#?}");
    let code = function("function m(o) { return o.f(1, 2); }", "m");
    // The method is read from the receiver in place; the receiver is then
    // copied to the base of the call window.
    assert_eq!(
        code[..5],
        [
            "GetField t1, p0, .f, ic0".to_owned(),
            "Mov t0, p0".to_owned(),
            "LoadInt t2, 1".to_owned(),
            "LoadInt t3, 2".to_owned(),
            "CallMethod t0, t0, 2".to_owned(),
        ]
    );
}

#[test]
fn constructors_record_their_this_slots() {
    let units = disassemble("function P(i) { this.x = i; this.y = i; this.x = 2; } new P(1);");
    let constructor = units.iter().find(|unit| unit.starts_with("P:")).unwrap();
    assert!(constructor.contains("this slots 2"), "{constructor}");
    let root = &units[0];
    // The result of the statement retargets into the script's completion
    // local.
    assert!(root.contains("New                 l0, t0, 1, ic"), "{root}");
}

#[test]
fn literals_reserve_their_slots_and_elements() {
    let code = function(
        "function f(a) { var o = {x: 1, y: a}; return [a, 2, o]; }",
        "f",
    );
    assert_eq!(code[0], "NewObject t0, 2");
    assert!(
        code.contains(&"DefineField t0, p0, .y".to_owned()),
        "{code:#?}"
    );
    assert!(
        code.contains(&"NewArray t0, [t0..t2]".to_owned()),
        "{code:#?}"
    );
}

#[test]
fn finally_runs_on_every_exit_path() {
    let code = function(
        "function f(x) { try { if (x) return 1; g(); } catch (e) { return e; } finally { h(); } return 2; }",
        "f",
    );
    let gosubs = code.iter().filter(|line| line.starts_with("Gosub")).count();
    // The return in try, normal completion of try, return in catch, normal
    // completion of catch, and the rethrow path.
    assert!(gosubs >= 4, "{code:#?}");
    assert!(
        code.iter().any(|line| line.starts_with("Ret ")),
        "{code:#?}"
    );
    let handlers: Vec<_> = code
        .iter()
        .filter(|line| line.starts_with("handler"))
        .collect();
    assert!(handlers.len() >= 2, "{code:#?}");
}

#[test]
fn nested_handlers_are_innermost_first_at_their_depths() {
    let source = "function f(a) { var r = [a, (function () { \
                  try { try { throw 1; } catch (e) { throw e + 1; } } catch (e2) { return e2; } })()]; \
                  return r; }";
    let units = disassemble(source);
    let inner = units.last().unwrap();
    let handlers: Vec<&str> = inner
        .lines()
        .filter(|line| line.contains("handler"))
        .collect();
    // The inner try, the inner catch body (whose scope exit is a region of
    // its own) and the outer try.
    assert_eq!(handlers.len(), 3, "{inner}");
    // The inner handlers resume above the outer marker.
    assert!(
        handlers[0].ends_with(", t1") && handlers[1].ends_with(", t1"),
        "{inner}"
    );
    assert!(handlers[2].ends_with(", t0"), "{inner}");
    let range = |line: &str| -> (u32, u32) {
        let range = line.split_whitespace().nth(1).unwrap();
        let (start, end) = range.split_once("..").unwrap();
        (start[1..].parse().unwrap(), end[1..].parse().unwrap())
    };
    let (inner_range, outer_range) = (range(handlers[0]), range(handlers[2]));
    assert!(outer_range.0 <= inner_range.0 && inner_range.1 <= outer_range.1);
}

#[test]
fn handler_resumes_above_live_operands() {
    // The array literal's first element is live below the try expression.
    let code = function(
        "function f(a) { return [a, (() => { try { return g(); } catch (e) { return 0; } })()]; }",
        "f",
    );
    assert!(
        code.iter().any(|line| line.starts_with("MakeClosure")),
        "{code:#?}"
    );
}

#[test]
fn unsupported_constructs_are_reported_not_mislowered() {
    for (source, construct) in [
        ("function* g() { yield 1; }", "generators"),
        ("class C {}", "classes"),
        ("for (const x of [1]) {}", "iterators"),
        ("function f(...r) { return r; }", "rest parameters"),
        ("var re = /a/;", "RegExp"),
        ("function f() { return arguments; }", "Arguments"),
    ] {
        match lower(source) {
            Err(LowerError::Unsupported(found)) => {
                assert!(found.contains(construct), "{source}: {found}");
            }
            Err(other) => panic!("{source}: {other}"),
            Ok(_) => panic!("{source}: lowered"),
        }
    }
}

fn max_stack(function: &UnlinkedFunction) -> u32 {
    u32::from(
        compute_stack_size(function.code(), u16::MAX)
            .unwrap()
            .max_stack,
    )
}

/// Pre-order, matching `CodeStore::tree`.
fn functions(function: &UnlinkedFunction) -> Vec<&UnlinkedFunction> {
    let mut order = vec![function];
    for constant in function.constants() {
        if let Some(child) = constant.as_child() {
            order.extend(functions(child));
        }
    }
    order
}

/// Lowers every JavaScript file of the repository's oracle cases and probes.
/// Each either lowers to units that pass the validator and whose register
/// count is `1 + params + locals + max depth` (plus one scratch temporary for
/// a capture's TDZ check), or is reported unsupported; nothing else.
#[test]
fn corpus_lowers_or_is_reported_unsupported() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    for directory in [
        "apps/cli/tests/oracle/cases",
        "apps/cli/tests/fixtures/inputs",
        "docs/performance/probes",
    ] {
        collect_js(&root.join(directory), &mut files);
    }
    files.sort();
    assert!(files.len() > 150, "found {} files", files.len());
    let (mut lowered, mut unsupported) = (0, 0);
    let mut reasons = std::collections::BTreeMap::new();
    for path in &files {
        let source = std::fs::read_to_string(path).unwrap();
        let Ok(script) = compile_unlinked_script(&source) else {
            continue;
        };
        let mut runtime = runtime();
        match lower_script(&mut runtime, &script) {
            Ok(result) => {
                lowered += 1;
                let order = runtime.code().tree(result.root);
                for (unit, function) in order.iter().zip(functions(&script)) {
                    let unit = runtime.code().get(*unit);
                    let depth = u32::from(unit.register_count) - unit.temp_base();
                    let stack = max_stack(function);
                    assert!(
                        depth == stack || depth == stack + 1,
                        "{}: depth {depth}, stack {stack}",
                        path.display()
                    );
                }
            }
            Err(LowerError::Unsupported(construct)) => {
                unsupported += 1;
                *reasons.entry(construct).or_insert(0) += 1;
            }
            Err(error) => panic!("{}: {error}", path.display()),
        }
    }
    eprintln!("corpus: {lowered} lowered, {unsupported} unsupported: {reasons:?}");
    assert!(lowered > 50, "lowered {lowered}, unsupported {unsupported}");
}

/// The fixed probes inside the phase 2 surface all lower.
#[test]
fn fixed_probes_lower() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/performance/probes/fixed");
    for name in [
        "empty_loop",
        "numeric_local_loop",
        "local_read",
        "local_read_with_setup",
        "local_move",
        "argument_move",
        "call0",
        "prop_read",
        "prop_write",
        "array_read",
        "array_write",
        "tdz",
        "type_error",
    ] {
        let path = directory.join(format!("{name}.js"));
        let source = std::fs::read_to_string(&path).unwrap();
        if let Err(error) = lower(&source) {
            panic!("{name}: {error}");
        }
    }
}

fn collect_js(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_js(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "js") {
            files.push(path);
        }
    }
}

#[test]
fn print_probe_disassembly() {
    let Ok(names) = std::env::var("DUMP_PROBES") else {
        return;
    };
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/performance/probes/fixed");
    for name in names.split(',') {
        let source = std::fs::read_to_string(directory.join(format!("{name}.js"))).unwrap();
        println!("==== {name}\n{source}");
        for unit in disassemble(&source) {
            println!("{unit}");
        }
    }
}
