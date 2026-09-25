//! Manual, test-only capture of published canonical bytecode.
//! Included by run_dump.py in a disposable worktree; not a production module.
use crate::engine::api::{compile::Compilation, Runtime};
use crate::engine::code::bytecode::Instruction;
use crate::engine::code::function::metadata::ClosureVariableKind;
use crate::engine::heap::{BytecodeConstant, FunctionBytecodeId, Heap};
use crate::engine::value::JsString;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

const TARGETS: [&str; 4] = ["am3", "project", "lin_solve", "advect"];

fn operand(instruction: &Instruction) -> String {
    match instruction {
        Instruction::GetLocal(index) => format!("L({index})"),
        Instruction::GetLocalCheck(index) => format!("LCheck({index})"),
        Instruction::GetArg(index) => format!("P({index})"),
        Instruction::PushI32(value) => format!("I32({value})"),
        Instruction::PushConst(index) => format!("K({index})"),
        _ => "-".to_owned(),
    }
}

fn is_direct(
    instruction: &Instruction,
    locals: &[crate::engine::code::function::metadata::VariableDefinition],
) -> bool {
    match instruction {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => locals
            .get(usize::from(*index))
            .is_some_and(|local| local.kind == ClosureVariableKind::Normal),
        Instruction::GetArg(_) => true,
        _ => false,
    }
}

fn is_numeric_source(
    instruction: &Instruction,
    locals: &[crate::engine::code::function::metadata::VariableDefinition],
    constants: &[BytecodeConstant],
) -> bool {
    match instruction {
        Instruction::PushI32(_) => true,
        Instruction::PushConst(index) => matches!(
            constants.get(*index as usize),
            Some(BytecodeConstant::Value(
                crate::engine::heap::RawValue::Int(_) | crate::engine::heap::RawValue::Float(_)
            ))
        ),
        _ => is_direct(instruction, locals),
    }
}

fn walk(
    heap: &Heap,
    id: FunctionBytecodeId,
    path: &str,
    source: &str,
    out: &mut File,
    manifest: &mut File,
    counts: &mut [usize; 4],
) -> io::Result<()> {
    let data = heap.function_bytecode(id).map_err(|error| {
        io::Error::other(format!("cannot read published function {path}: {error:?}"))
    })?;
    let target = TARGETS.iter().position(|name| {
        data.func_name
            .as_ref()
            .is_some_and(|actual| actual == &JsString::from_static(*name))
    });
    if let Some(target) = target {
        counts[target] += 1;
        writeln!(out, "FUNCTION\t{}\t{}\t{:?}", TARGETS[target], path, id)?;
        writeln!(out, "METADATA\t{:?}", data.metadata)?;
        writeln!(out, "ARGUMENTS\t{:?}", data.argument_definitions)?;
        writeln!(out, "LOCALS\t{:?}", data.local_definitions)?;
        writeln!(out, "CLOSURES\t{:?}", data.closure_variables)?;
        for (index, value) in data.constants.iter().enumerate() {
            writeln!(out, "CONSTANT\t{index}\t{value:?}")?;
        }
        for (pc, instruction) in data.code.iter().enumerate() {
            let stack = instruction.stack_contract();
            let control = instruction.control_effect();
            writeln!(
                out,
                "OP\t{pc}\t{instruction:?}\tpop={}\tpush={}\ttarget={:?}\tends_block={}",
                stack.popped,
                stack.pushed,
                control.target(),
                control.ends_block(),
            )?;
        }
        // Every GetArrayEl is considered as the tail of an R0 triad. The
        // accepted flag comes from the published production FusionPlan, not
        // from a second matcher or source-text search.
        for end in 0..data.code.len() {
            if !matches!(data.code[end], Instruction::GetArrayEl) {
                continue;
            }
            let Some(first) = end.checked_sub(2) else {
                writeln!(
                    manifest,
                    "{source}\t{path}\t-\t{end}\t-\t-\t-\t-\t-\t-\t-\tmissing_producers"
                )?;
                continue;
            };
            let ops = &data.code[first..=end];
            let accepted = data.fusion.dense_span(first) == Some(super::DenseSpanKind::Read);
            let reason = if accepted {
                "accepted"
            } else if !is_direct(&ops[0], &data.local_definitions) {
                "base_not_direct"
            } else if !is_numeric_source(&ops[1], &data.local_definitions, &data.constants) {
                "key_not_numeric_source"
            } else {
                // Both producers and the tail have the R0 shape. An
                // unpublished site has an authenticated internal entry.
                "internal_entry"
            };
            let opcodes = ops
                .iter()
                .map(|op| format!("{op:?}"))
                .collect::<Vec<_>>()
                .join(";");
            writeln!(
                manifest,
                "{source}\t{path}\t{first}\t{end}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{reason}",
                if accepted { "1" } else { "-" },
                operand(&ops[0]),
                operand(&ops[1]),
                opcodes,
                if accepted { "2" } else { "-" },
                if accepted { "1" } else { "-" },
                "R0",
            )?;
        }
        writeln!(out, "END_FUNCTION\t{}", TARGETS[target])?;
    }
    // The root is held for this entire walk; children are owned constant edges.
    // No retain/release or mutable runtime borrow is needed for inspection.
    for (index, constant) in data.constants.iter().enumerate() {
        if let BytecodeConstant::Function(child) = constant {
            walk(
                heap,
                *child,
                &format!("{path}/constant[{index}]"),
                source,
                out,
                manifest,
                counts,
            )?;
        }
    }
    Ok(())
}

#[test]
#[ignore = "manual external-source bytecode capture; use docs/performance/probes/run_dump.py"]
fn dump_numeric_spans() {
    let source = PathBuf::from(std::env::var_os("OXIDE_DUMP_SOURCE").expect("OXIDE_DUMP_SOURCE"));
    let output = PathBuf::from(std::env::var_os("OXIDE_DUMP_OUTPUT").expect("OXIDE_DUMP_OUTPUT"));
    let mut counts = [0_usize; 4];
    let mut manifest = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("dense-sites.tsv"))
        .expect("create fresh site manifest");
    writeln!(manifest, "source\tfunction_path\tfirst_pc\tlast_pc\tflag\tbase_slot\tkey_source\topcodes\tpeak\tdelta\tkind\trejection_reason").unwrap();
    for name in ["crypto", "navier-stokes"] {
        let path = source.join("v8-v7").join(format!("{name}.js"));
        let text = std::fs::read_to_string(&path).expect("read complete pinned source");
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let compilation = runtime
            .compile_in_realm(context.realm, &text, &format!("v8-v7/{name}.js"))
            .expect("compile and publish complete source");
        let root = match compilation {
            Compilation::Published(root) => root,
            Compilation::Throw(value) => {
                runtime
                    .release_jsvalue(value)
                    .expect("release compilation exception");
                panic!("source compilation threw: {name}");
            }
        };
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(format!("{name}.canonical.txt")))
            .expect("create fresh canonical output");
        writeln!(out, "FORMAT\toxide-published-canonical-v1").unwrap();
        writeln!(out, "SOURCE\tv8-v7/{name}.js").unwrap();
        {
            let state = runtime.0.state.borrow();
            walk(
                &state.heap,
                root.bytecode_id(),
                name,
                &format!("v8-v7/{name}.js"),
                &mut out,
                &mut manifest,
                &mut counts,
            )
            .expect("walk published bytecode while root is alive");
        }
        out.sync_all().unwrap();
        // Neither the script nor any benchmark function has been executed.
    }
    assert_eq!(counts, [1, 1, 1, 1], "missing or ambiguous target function");
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("target-counts.tsv"))
        .expect("create target count receipt");
    for (name, count) in TARGETS.iter().zip(counts) {
        writeln!(out, "{name}\t{count}").unwrap();
    }
    out.sync_all().unwrap();
    manifest.sync_all().unwrap();
}
