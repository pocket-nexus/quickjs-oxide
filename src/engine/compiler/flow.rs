//! Required stack-state validation and entry facts used during lowering.
//!
//! The code verifier remains authoritative for reachable normal, exception,
//! private-reference and resume states. This adapter never replaces it with
//! a syntactic stack-height estimate or a budget-limited dataflow result.

use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::code::bytecode::{Instruction, verify_parts};
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::code::function::UnlinkedVariableDefinition;
use crate::engine::code::function::metadata::ClosureVariableKind;
use crate::engine::code::instruction::{PotentialEffects, StackStateEffect};
use crate::engine::code::region::{DirectSource, NumberSource, NumericOperation, NumericRegion};
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
    input_start: usize,
    input_len: usize,
    uses: usize,
    effects: PotentialEffects,
}

fn inputs_of<'a>(nodes: &[UseNode], inputs: &'a [usize], node: usize) -> &'a [usize] {
    let node = &nodes[node];
    &inputs[node.input_start..node.input_start + node.input_len]
}

fn direct_source(
    instruction: &Instruction,
    locals: &[UnlinkedVariableDefinition],
    captured: &[bool],
    argument_count: usize,
    initialized: &[bool],
) -> Option<DirectSource> {
    match *instruction {
        Instruction::GetLocal(slot) | Instruction::GetLocalCheck(slot) => {
            let definition = locals.get(slot as usize)?;
            if definition.kind != ClosureVariableKind::Normal
                || captured.get(slot as usize).copied().unwrap_or(true)
                || definition.is_lexical != matches!(instruction, Instruction::GetLocalCheck(_))
                || definition.is_lexical
                    && !initialized.get(slot as usize).copied().unwrap_or(false)
            {
                return None;
            }
            Some(if definition.is_lexical {
                DirectSource::CheckedLocal(slot)
            } else {
                DirectSource::Local(slot)
            })
        }
        Instruction::GetArg(slot) if (slot as usize) < argument_count => {
            Some(DirectSource::Argument(slot))
        }
        _ => None,
    }
}

fn numeric_source(
    instruction: &Instruction,
    constants: &[IrConstant],
    direct: Option<DirectSource>,
) -> Option<NumberSource> {
    match *instruction {
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
        _ => direct.map(NumberSource::Direct),
    }
}

fn numeric_destination(
    instruction: &Instruction,
    locals: &[UnlinkedVariableDefinition],
    captured: &[bool],
    initialized: &[bool],
) -> Option<(u16, bool)> {
    let (index, checked) = match *instruction {
        Instruction::SetLocal(index) => (index, false),
        Instruction::SetLocalCheck(index) => (index, true),
        _ => return None,
    };
    let definition = locals.get(index as usize)?;
    (definition.kind == ClosureVariableKind::Normal
        && !definition.is_const
        && !captured.get(index as usize).copied().unwrap_or(true)
        && definition.is_lexical == checked
        && (!checked || initialized.get(index as usize).copied().unwrap_or(false)))
    .then_some((index, checked))
}

fn region_peak(code: &[Instruction], start: usize, end: usize, entries: &[bool]) -> Option<u16> {
    if start >= end || end >= code.len() || entries.get(start + 1..end)?.iter().any(|&entry| entry)
    {
        return None;
    }
    let mut depth = 0usize;
    let mut peak = 0usize;
    for instruction in &code[start..end] {
        let effect = instruction.stack_contract();
        if effect.state != StackStateEffect::Ordinary {
            return None;
        }
        depth = depth
            .checked_sub(effect.popped)?
            .checked_add(effect.pushed)?;
        peak = peak.max(depth);
    }
    (depth == 0).then(|| u16::try_from(peak).ok()).flatten()
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
    if dynamic_bindings
        || !code
            .iter()
            .any(|op| matches!(op, Instruction::GetArrayEl | Instruction::GetArrayEl3))
    {
        return Vec::new();
    }
    let entries = block_entries(code);
    let mut initialized = vec![false; locals.len()];
    let facts = RegionFacts {
        code,
        constants,
        locals,
        captured,
        argument_count,
        entries: &entries,
    };
    let mut nodes: Vec<UseNode> = Vec::new();
    let mut inputs: Vec<usize> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut selected = Vec::new();
    let mut occupied_until = 0;
    for (pc, instruction) in code.iter().enumerate() {
        if entries[pc] {
            stack.clear();
            nodes.clear();
            inputs.clear();
            initialized.fill(false);
        }
        let effect = instruction.stack_contract();
        if effect.state != StackStateEffect::Ordinary || stack.len() < effect.popped {
            stack.clear();
            nodes.clear();
            inputs.clear();
            continue;
        }
        let popped_start = stack.len() - effect.popped;
        let input_start = inputs.len();
        inputs.extend_from_slice(&stack[popped_start..]);
        for &input in &stack[popped_start..] {
            nodes[input].uses += 1;
        }
        stack.truncate(popped_start);
        let node = nodes.len();
        nodes.push(UseNode {
            pc,
            input_start,
            input_len: effect.popped,
            uses: 0,
            effects: instruction.potential_effects(),
        });
        if pc >= occupied_until {
            let region = if matches!(instruction, Instruction::Drop) {
                numeric_region_at_drop(facts, &nodes, &inputs, &initialized, node)
                    .or_else(|| store_product_at_drop(facts, &nodes, &inputs, &initialized, node))
                    .or_else(|| update_element_at_drop(facts, &initialized, pc))
            } else if matches!(
                instruction,
                Instruction::IfTrue(_) | Instruction::IfFalse(_)
            ) {
                compare_branch_at(facts, &initialized, pc)
            } else {
                None
            };
            if let Some(region) = region {
                if region.start as usize >= occupied_until {
                    occupied_until = region.end as usize;
                    selected.push(region);
                }
            }
        }
        stack.extend(std::iter::repeat_n(node, effect.pushed));
        match instruction {
            Instruction::InitializeLocal(index) => {
                if let Some(value) = initialized.get_mut(usize::from(*index)) {
                    *value = true;
                }
            }
            Instruction::SetLocalUninitialized(index) => {
                if let Some(value) = initialized.get_mut(usize::from(*index)) {
                    *value = false;
                }
            }
            _ => {}
        }
        if instruction.control_effect().ends_block() {
            stack.clear();
        }
    }
    selected
}

fn numeric_region_at_drop(
    facts: RegionFacts<'_>,
    nodes: &[UseNode],
    inputs: &[usize],
    initialized: &[bool],
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
    let single = |node: usize| inputs_of(nodes, inputs, node).first().copied();
    let store = single(drop)?;
    let (destination, checked) =
        numeric_destination(&code[nodes[store].pc], locals, captured, initialized)?;
    let add = single(store)?;
    if !matches!(code[nodes[add].pc], Instruction::Add) {
        return None;
    }
    let [old, product] = inputs_of(nodes, inputs, add) else {
        return None;
    };
    if !matches!(code[nodes[*old].pc], Instruction::GetLocal(index) if !checked && index == destination)
        && !matches!(code[nodes[*old].pc], Instruction::GetLocalCheck(index) if checked && index == destination)
        || !matches!(code[nodes[*product].pc], Instruction::Mul)
    {
        return None;
    }
    let [element, scale] = inputs_of(nodes, inputs, *product) else {
        return None;
    };
    if !matches!(code[nodes[*element].pc], Instruction::GetArrayEl) {
        return None;
    }
    let [array, index] = inputs_of(nodes, inputs, *element) else {
        return None;
    };
    let direct = |node: usize| {
        direct_source(
            &code[nodes[node].pc],
            locals,
            captured,
            argument_count,
            initialized,
        )
    };
    let number_source =
        |node: usize| numeric_source(&code[nodes[node].pc], constants, direct(node));
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
                    | Instruction::GetLocalCheck(_)
                    | Instruction::GetArg(_)
                    | Instruction::PushConst(_)
                    | Instruction::GetArrayEl
                    | Instruction::Mul
                    | Instruction::Add
                    | Instruction::SetLocal(_)
                    | Instruction::SetLocalCheck(_)
            )
        {
            return None;
        }
    }
    let peak = region_peak(code, start, nodes[drop].pc + 1, entries)?;
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: (nodes[drop].pc + 1).try_into().ok()?,
        array: array_source,
        index: index_source,
        operation: NumericOperation::Accumulate {
            destination,
            scale: scale_source,
            checked,
        },
        peak,
    })
}

fn store_product_at_drop(
    facts: RegionFacts<'_>,
    nodes: &[UseNode],
    inputs: &[usize],
    initialized: &[bool],
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
    let single = |node: usize| inputs_of(nodes, inputs, node).first().copied();
    let store = single(drop)?;
    let (destination, checked) =
        numeric_destination(&code[nodes[store].pc], locals, captured, initialized)?;
    let product = single(store)?;
    if !matches!(code[nodes[product].pc], Instruction::Mul) {
        return None;
    }
    let [element, scale_node] = inputs_of(nodes, inputs, product) else {
        return None;
    };
    if !matches!(code[nodes[*element].pc], Instruction::GetArrayEl) {
        return None;
    }
    let [array_node, index_node] = inputs_of(nodes, inputs, *element) else {
        return None;
    };
    let source = |node: usize| {
        direct_source(
            &code[nodes[node].pc],
            locals,
            captured,
            argument_count,
            initialized,
        )
    };
    let number = |node: usize| numeric_source(&code[nodes[node].pc], constants, source(node));
    let array = source(*array_node)?;
    let index = number(*index_node)?;
    let scale = number(*scale_node)?;
    let members = [
        *array_node,
        *index_node,
        *element,
        *scale_node,
        product,
        store,
        drop,
    ];
    let mut pcs = members.map(|id| nodes[id].pc);
    pcs.sort_unstable();
    let start = pcs[0];
    if pcs
        .iter()
        .enumerate()
        .any(|(offset, &pc)| pc != start + offset)
        || nodes[drop].pc + 1 != start + members.len()
        || members.iter().any(|&id| id != drop && nodes[id].uses != 1)
    {
        return None;
    }
    let end = nodes[drop].pc + 1;
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: end.try_into().ok()?,
        array,
        index,
        operation: NumericOperation::StoreProduct {
            destination,
            scale,
            checked,
        },
        peak: region_peak(code, start, end, entries)?,
    })
}

fn update_element_at_drop(
    facts: RegionFacts<'_>,
    initialized: &[bool],
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
    let start = drop.checked_sub(7)?;
    let span = code.get(start..=drop)?;
    if !matches!(span[2], Instruction::GetArrayEl3)
        || !matches!(span[4], Instruction::Add)
        || !matches!(span[5], Instruction::Insert3)
        || !matches!(span[6], Instruction::PutArrayEl)
        || !matches!(span[7], Instruction::Drop)
    {
        return None;
    }
    let direct = |instruction: &Instruction| {
        direct_source(instruction, locals, captured, argument_count, initialized)
    };
    let array = direct(&span[0])?;
    let index = numeric_source(&span[1], constants, direct(&span[1]))?;
    let delta = numeric_source(&span[3], constants, direct(&span[3]))?;
    let end = drop + 1;
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: end.try_into().ok()?,
        array,
        index,
        operation: NumericOperation::UpdateElement { delta },
        peak: region_peak(code, start, end, entries)?,
    })
}

fn compare_branch_at(
    facts: RegionFacts<'_>,
    initialized: &[bool],
    branch: usize,
) -> Option<NumericRegion> {
    let RegionFacts {
        code,
        constants,
        locals,
        captured,
        argument_count,
        entries,
    } = facts;
    let start = branch.checked_sub(5)?;
    let span = code.get(start..=branch)?;
    if !matches!(span[2], Instruction::GetArrayEl) {
        return None;
    }
    let comparison = Opcode::from_instruction(&span[4]);
    if !matches!(
        comparison,
        Opcode::Lt
            | Opcode::Lte
            | Opcode::Gt
            | Opcode::Gte
            | Opcode::Eq
            | Opcode::Neq
            | Opcode::StrictEq
            | Opcode::StrictNeq
    ) {
        return None;
    }
    let (when_true, target) = match span[5] {
        Instruction::IfTrue(target) => (true, target),
        Instruction::IfFalse(target) => (false, target),
        _ => return None,
    };
    let target_index = usize::try_from(target).ok()?;
    if (start + 1..=branch).contains(&target_index) {
        return None;
    }
    let direct = |instruction: &Instruction| {
        direct_source(instruction, locals, captured, argument_count, initialized)
    };
    let array = direct(&span[0])?;
    let index = numeric_source(&span[1], constants, direct(&span[1]))?;
    let rhs = numeric_source(&span[3], constants, direct(&span[3]))?;
    let end = branch + 1;
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: end.try_into().ok()?,
        array,
        index,
        operation: NumericOperation::CompareBranch {
            rhs,
            comparison,
            when_true,
            target,
        },
        peak: region_peak(code, start, end, entries)?,
    })
}
