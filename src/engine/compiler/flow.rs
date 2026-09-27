//! Required stack-state validation and entry facts used during lowering.
//!
//! The code verifier remains authoritative for reachable normal, exception,
//! private-reference and resume states. This adapter never replaces it with
//! a syntactic stack-height estimate or a budget-limited dataflow result.

use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::code::bytecode::{Instruction, verify_parts};
use crate::engine::code::function::UnlinkedVariableDefinition;
use crate::engine::code::function::metadata::ClosureVariableKind;
use crate::engine::code::instruction::{PotentialEffects, StackStateEffect};
use crate::engine::code::region::{DirectSource, NumberSource, NumericRegion};
use crate::engine::compiler::MAX_BYTECODE_STACK;
use crate::engine::compiler::model::ir::IrConstant;
use crate::engine::value::PrimitiveValue;
use crate::engine::value::number::operations::Number;

pub(super) fn verify_lowered_max_stack(
    code: &[Instruction],
    constant_count: usize,
) -> Result<u16, Error> {
    verify_parts(code, constant_count, MAX_BYTECODE_STACK as u16)
        .map(|verified| verified.max_stack)
        .map_err(|error| {
            if matches!(
                error.message(),
                "declared maximum stack is smaller than required"
                    | "bytecode stack exceeds u16::MAX"
            ) {
                Error::new(ErrorKind::JsInternal, "stack overflow")
            } else {
                error
            }
        })
}

/// Structural basic-block starts, including unreachable continuations. These
/// bits delimit local rewrites; they do not assert reachability, initialized
/// locals, or a valid resume shape. `verify_parts` proves those stack facts.
pub(super) fn block_entries(code: &[Instruction]) -> Vec<bool> {
    #[cfg(feature = "profiling")]
    let _phase_timer = crate::engine::api::profiling::PhaseTimer::start(
        crate::engine::api::profiling::CompilePhase::Blocks,
    );
    let mut entries = vec![false; code.len()];
    if let Some(entry) = entries.first_mut() {
        *entry = true;
    }
    for (pc, instruction) in code.iter().enumerate() {
        let control = instruction.control_effect();
        if let Some(target) = control.target()
            && let Ok(target) = usize::try_from(target)
            && let Some(entry) = entries.get_mut(target)
        {
            *entry = true;
        }
        if control.ends_block()
            && let Some(entry) = entries.get_mut(pc + 1)
        {
            *entry = true;
        }
    }
    entries
}

/// A short-lived basic-block use graph. Its nodes describe value producers,
/// including effects and consumed values; only descriptors survive lowering.
struct UseNode {
    pc: usize,
    inputs: Vec<usize>,
    uses: usize,
    effects: PotentialEffects,
}

#[derive(Clone, Copy)]
struct RegionFacts<'a> {
    code: &'a [Instruction],
    constants: &'a [IrConstant],
    locals: &'a [UnlinkedVariableDefinition],
    captured: &'a [bool],
    argument_count: usize,
    entries: &'a [bool],
}

pub(super) fn plan_numeric_regions(
    code: &[Instruction],
    constants: &[IrConstant],
    locals: &[UnlinkedVariableDefinition],
    captured: &[bool],
    argument_count: usize,
    dynamic_bindings: bool,
) -> Vec<NumericRegion> {
    if dynamic_bindings || !code.iter().any(|op| matches!(op, Instruction::GetArrayEl)) {
        return Vec::new();
    }
    let entries = block_entries(code);
    let facts = RegionFacts {
        code,
        constants,
        locals,
        captured,
        argument_count,
        entries: &entries,
    };
    let mut nodes: Vec<UseNode> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut selected = Vec::new();
    let mut occupied_until = 0;
    for (pc, instruction) in code.iter().enumerate() {
        if entries[pc] {
            stack.clear();
        }
        let effect = instruction.stack_contract();
        if effect.state != StackStateEffect::Ordinary || stack.len() < effect.popped {
            stack.clear();
            continue;
        }
        let inputs = stack.split_off(stack.len() - effect.popped);
        for &input in &inputs {
            nodes[input].uses += 1;
        }
        let node = nodes.len();
        nodes.push(UseNode {
            pc,
            inputs,
            uses: 0,
            effects: instruction.potential_effects(),
        });
        if matches!(instruction, Instruction::Drop) && pc >= occupied_until {
            if let Some(region) = numeric_region_at_drop(facts, &nodes, node) {
                if region.start as usize >= occupied_until {
                    occupied_until = region.end as usize;
                    selected.push(region);
                }
            }
        }
        stack.extend(std::iter::repeat_n(node, effect.pushed));
        if instruction.control_effect().ends_block() {
            stack.clear();
        }
    }
    selected
}

fn numeric_region_at_drop(
    facts: RegionFacts<'_>,
    nodes: &[UseNode],
    drop: usize,
) -> Option<NumericRegion> {
    let RegionFacts {
        code,
        constants,
        locals,
        captured,
        argument_count,
        entries,
    } = facts;
    let single = |node: usize| nodes.get(node)?.inputs.as_slice().first().copied();
    let store = single(drop)?;
    let Instruction::SetLocal(destination) = code[nodes[store].pc] else {
        return None;
    };
    let definition = locals.get(destination as usize)?;
    if definition.kind != ClosureVariableKind::Normal
        || definition.is_lexical
        || definition.is_const
        || *captured.get(destination as usize)?
    {
        return None;
    }
    let add = single(store)?;
    if !matches!(code[nodes[add].pc], Instruction::Add) {
        return None;
    }
    let [old, product] = nodes[add].inputs.as_slice() else {
        return None;
    };
    if !matches!(code[nodes[*old].pc], Instruction::GetLocal(index) if index == destination)
        || !matches!(code[nodes[*product].pc], Instruction::Mul)
    {
        return None;
    }
    let [element, scale] = nodes[*product].inputs.as_slice() else {
        return None;
    };
    if !matches!(code[nodes[*element].pc], Instruction::GetArrayEl) {
        return None;
    }
    let [array, index] = nodes[*element].inputs.as_slice() else {
        return None;
    };
    let direct = |node: usize| match code[nodes[node].pc] {
        Instruction::GetLocal(slot)
            if locals.get(slot as usize).is_some_and(|definition| {
                definition.kind == ClosureVariableKind::Normal
                    && !definition.is_lexical
                    && !captured.get(slot as usize).copied().unwrap_or(true)
            }) =>
        {
            Some(DirectSource::Local(slot))
        }
        Instruction::GetArg(slot) if (slot as usize) < argument_count => {
            Some(DirectSource::Argument(slot))
        }
        _ => None,
    };
    let number_source = |node: usize| match code[nodes[node].pc] {
        Instruction::PushI32(value) => Some(NumberSource::Immediate(value)),
        Instruction::PushConst(index) => match constants.get(index as usize)? {
            IrConstant::Primitive(PrimitiveValue::Int(value)) => Some(NumberSource::Constant {
                index,
                value: Number::Int(*value),
            }),
            IrConstant::Primitive(PrimitiveValue::Float(value)) => Some(NumberSource::Constant {
                index,
                value: Number::Float(*value),
            }),
            _ => None,
        },
        _ => direct(node).map(NumberSource::Direct),
    };
    let array_source = direct(*array)?;
    let index_source = number_source(*index)?;
    let scale_source = number_source(*scale)?;
    let member_ids = [
        *old, *array, *index, *element, *scale, *product, add, store, drop,
    ];
    let mut member_pcs = member_ids.map(|id| nodes[id].pc);
    member_pcs.sort_unstable();
    let start = member_pcs[0];
    if member_pcs
        .iter()
        .enumerate()
        .any(|(offset, &pc)| pc != start + offset)
        || entries[start + 1..=nodes[drop].pc]
            .iter()
            .any(|&entry| entry)
        || nodes[drop].pc + 1 != start + member_pcs.len()
    {
        return None;
    }
    for &id in &member_ids {
        if id != drop && nodes[id].uses != 1 {
            return None;
        }
        // Every generic effect inside the region must be one of the four
        // operations whose Number/direct-slot guard discharges it at runtime.
        let effects = nodes[id].effects;
        if (effects.may_call_js || effects.may_allocate)
            && !matches!(
                code[nodes[id].pc],
                Instruction::GetLocal(_)
                    | Instruction::GetArg(_)
                    | Instruction::PushConst(_)
                    | Instruction::GetArrayEl
                    | Instruction::Mul
                    | Instruction::Add
                    | Instruction::SetLocal(_)
            )
        {
            return None;
        }
    }
    let mut depth = 0usize;
    let mut peak = 0usize;
    for instruction in &code[start..=nodes[drop].pc] {
        let effect = instruction.stack_contract();
        depth = depth
            .checked_sub(effect.popped)?
            .checked_add(effect.pushed)?;
        peak = peak.max(depth);
    }
    if depth != 0 {
        return None;
    }
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: (nodes[drop].pc + 1).try_into().ok()?,
        destination,
        array: array_source,
        index: index_source,
        scale: scale_source,
        peak: peak.try_into().ok()?,
    })
}
