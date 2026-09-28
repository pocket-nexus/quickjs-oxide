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
use crate::engine::code::initialization::definite_initialization_entries;
use crate::engine::code::instruction::{PotentialEffects, StackStateEffect};
use crate::engine::code::region::{
    ArrayProductSource, ArrayReadSource, DirectSource, NumberSource, NumericOperation,
    NumericRegion, UpdateDelta,
};
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
    region_peak_with_stack(code, start, end, entries, 0, 0)
}

fn region_peak_with_stack(
    code: &[Instruction],
    start: usize,
    end: usize,
    entries: &[bool],
    input_depth: usize,
    output_depth: usize,
) -> Option<u16> {
    if start >= end || end >= code.len() || entries.get(start + 1..end)?.iter().any(|&entry| entry)
    {
        return None;
    }
    let mut depth = input_depth;
    let mut peak = input_depth;
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
    (depth == output_depth)
        .then(|| u16::try_from(peak - input_depth).ok())
        .flatten()
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
        || !code.iter().any(|op| {
            matches!(
                op,
                Instruction::GetArrayEl | Instruction::GetArrayEl3 | Instruction::PutArrayEl
            )
        })
    {
        return Vec::new();
    }
    let entries = block_entries(code);
    let initialization_entries = if locals.iter().any(|local| local.is_lexical) {
        definite_initialization_entries(code, locals.len())
    } else {
        vec![None; code.len()]
    };
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
            if let Some(state) = initialization_entries[pc].as_deref() {
                initialized.copy_from_slice(state);
            } else {
                initialized.fill(false);
            }
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
                    .or_else(|| copy_element_at_drop(facts, &initialized, pc))
                    .or_else(|| update_element_at_drop(facts, &nodes, &inputs, &initialized, node))
                    .or_else(|| store_element_and_local_at_drop(facts, &initialized, pc))
            } else if matches!(
                instruction,
                Instruction::IfTrue(_) | Instruction::IfFalse(_)
            ) {
                compare_branch_at(facts, &initialized, pc)
            } else if matches!(instruction, Instruction::Add) {
                add_preinc_at_add(facts, &initialized, pc)
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
    // Two adjacent AddPreInc fragments have one evolving stack Number. The
    // second read of an aliased index is the post-first-write version; all
    // admission still precedes either commit in the published operation.
    let mut composed = Vec::with_capacity(selected.len());
    let mut candidates = selected.into_iter().peekable();
    while let Some(first) = candidates.next() {
        if matches!(first.operation, NumericOperation::AddPreInc)
            && let Some(second) = candidates.peek()
            && matches!(second.operation, NumericOperation::AddPreInc)
            && first.end == second.start
            && !entries[second.start as usize]
            && let NumberSource::Direct(second_index) = second.index
        {
            let second = candidates.next().expect("peeked second region");
            let second_reads_updated_index = matches!(first.index, NumberSource::Direct(first_index) if first_index == second_index);
            composed.push(NumericRegion {
                end: second.end,
                operation: NumericOperation::AddPreIncPair {
                    second_array: second.array,
                    second_index,
                    second_reads_updated_index,
                },
                peak: first.peak.max(second.peak),
                ..first
            });
        } else {
            composed.push(first);
        }
    }
    composed
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
    let code = facts.code;
    let locals = facts.locals;
    let captured = facts.captured;
    let entries = facts.entries;
    let single = |node: usize| inputs_of(nodes, inputs, node).first().copied();
    let store = single(drop)?;
    let (destination, checked) =
        numeric_destination(&code[nodes[store].pc], locals, captured, initialized)?;
    let product = single(store)?;
    let (product_source, product_members) =
        array_product_source(facts, nodes, inputs, initialized, product)?;
    let members = [
        product_members[0],
        product_members[1],
        product_members[2],
        product_members[3],
        product_members[4],
        store,
        drop,
    ];
    // The published StoreProduct fallback replays the array read first. Keep
    // the shared product recognizer order-flexible for other consumers, but
    // only select this sink when its retained words have that exact order.
    let pcs = members.map(|id| nodes[id].pc);
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
        array: product_source.array,
        index: product_source.index,
        operation: NumericOperation::StoreProduct {
            destination,
            scale: product_source.scale,
            checked,
        },
        peak: region_peak(code, start, end, entries)?,
    })
}

/// A reusable producer fact: one Array element multiplied by a direct Number.
/// Callers decide separately whether that product feeds a local or an element.
fn array_product_source(
    facts: RegionFacts<'_>,
    nodes: &[UseNode],
    inputs: &[usize],
    initialized: &[bool],
    product: usize,
) -> Option<(ArrayProductSource, [usize; 5])> {
    let code = facts.code;
    if !matches!(code[nodes[product].pc], Instruction::Mul) {
        return None;
    }
    let [left, right] = inputs_of(nodes, inputs, product) else {
        return None;
    };
    let (element, scale_node) = if matches!(code[nodes[*left].pc], Instruction::GetArrayEl) {
        (*left, *right)
    } else if matches!(code[nodes[*right].pc], Instruction::GetArrayEl) {
        (*right, *left)
    } else {
        return None;
    };
    let [array_node, index_node] = inputs_of(nodes, inputs, element) else {
        return None;
    };
    let direct = |node: usize| {
        direct_source(
            &code[nodes[node].pc],
            facts.locals,
            facts.captured,
            facts.argument_count,
            initialized,
        )
    };
    let number = |node: usize| numeric_source(&code[nodes[node].pc], facts.constants, direct(node));
    Some((
        ArrayProductSource {
            array: direct(*array_node)?,
            index: number(*index_node)?,
            scale: number(scale_node)?,
        },
        [*array_node, *index_node, element, scale_node, product],
    ))
}

fn copy_element_at_drop(
    facts: RegionFacts<'_>,
    initialized: &[bool],
    drop: usize,
) -> Option<NumericRegion> {
    let start = drop.checked_sub(7)?;
    let span = facts.code.get(start..=drop)?;
    if !matches!(span[4], Instruction::GetArrayEl)
        || !matches!(span[5], Instruction::Insert3)
        || !matches!(span[6], Instruction::PutArrayEl)
        || !matches!(span[7], Instruction::Drop)
    {
        return None;
    }
    let direct = |instruction: &Instruction| {
        direct_source(
            instruction,
            facts.locals,
            facts.captured,
            facts.argument_count,
            initialized,
        )
    };
    let number = |instruction: &Instruction| {
        numeric_source(instruction, facts.constants, direct(instruction))
    };
    let end = drop + 1;
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: end.try_into().ok()?,
        array: direct(&span[0])?,
        index: number(&span[1])?,
        operation: NumericOperation::CopyElement {
            source: ArrayReadSource {
                array: direct(&span[2])?,
                index: number(&span[3])?,
            },
        },
        peak: region_peak(facts.code, start, end, facts.entries)?,
    })
}

fn add_preinc_at_add(
    facts: RegionFacts<'_>,
    initialized: &[bool],
    add: usize,
) -> Option<NumericRegion> {
    let start = add.checked_sub(5)?;
    let span = facts.code.get(start..=add)?;
    let (index, checked) =
        numeric_destination(&span[3], facts.locals, facts.captured, initialized)?;
    if !matches!(span[2], Instruction::Inc)
        || !matches!(span[4], Instruction::GetArrayEl)
        || !matches!(span[5], Instruction::Add)
        || !matches!(span[1], Instruction::GetLocal(value) if !checked && value == index)
            && !matches!(span[1], Instruction::GetLocalCheck(value) if checked && value == index)
    {
        return None;
    }
    let array = direct_source(
        &span[0],
        facts.locals,
        facts.captured,
        facts.argument_count,
        initialized,
    )?;
    let index = if checked {
        NumberSource::Direct(DirectSource::CheckedLocal(index))
    } else {
        NumberSource::Direct(DirectSource::Local(index))
    };
    let end = add + 1;
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: end.try_into().ok()?,
        array,
        index,
        operation: NumericOperation::AddPreInc,
        peak: region_peak_with_stack(facts.code, start, end, facts.entries, 1, 1)?,
    })
}

fn store_element_and_local_at_drop(
    facts: RegionFacts<'_>,
    initialized: &[bool],
    drop: usize,
) -> Option<NumericRegion> {
    let start = drop.checked_sub(3)?;
    let span = facts.code.get(start..=drop)?;
    if !matches!(span[0], Instruction::Insert3)
        || !matches!(span[1], Instruction::PutArrayEl)
        || !matches!(span[3], Instruction::Drop)
    {
        return None;
    }
    let (destination, checked) =
        numeric_destination(&span[2], facts.locals, facts.captured, initialized)?;
    let end = drop + 1;
    Some(NumericRegion {
        start: start.try_into().ok()?,
        end: end.try_into().ok()?,
        // This operation consumes its array and index from the operand stack.
        array: DirectSource::Local(0),
        index: NumberSource::Immediate(0),
        operation: NumericOperation::StoreElementAndLocal {
            destination,
            checked,
        },
        peak: region_peak_with_stack(facts.code, start, end, facts.entries, 3, 0)?,
    })
}

fn update_element_at_drop(
    facts: RegionFacts<'_>,
    nodes: &[UseNode],
    inputs: &[usize],
    initialized: &[bool],
    drop_node: usize,
) -> Option<NumericRegion> {
    let RegionFacts {
        code,
        constants,
        locals,
        captured,
        argument_count,
        entries,
    } = facts;
    let drop = nodes[drop_node].pc;
    let direct = |instruction: &Instruction| {
        direct_source(instruction, locals, captured, argument_count, initialized)
    };
    let (start, delta) = if let Some(start) = drop.checked_sub(7) {
        let span = code.get(start..=drop)?;
        if matches!(span[2], Instruction::GetArrayEl3)
            && matches!(span[4], Instruction::Add)
            && matches!(span[5], Instruction::Insert3)
            && matches!(span[6], Instruction::PutArrayEl)
            && matches!(span[7], Instruction::Drop)
        {
            let value = numeric_source(&span[3], constants, direct(&span[3]))?;
            (start, UpdateDelta::Number(value))
        } else {
            update_array_product_at_drop(facts, nodes, inputs, initialized, drop_node)?
        }
    } else {
        update_array_product_at_drop(facts, nodes, inputs, initialized, drop_node)?
    };
    let span = code.get(start..=drop)?;
    let array = direct(&span[0])?;
    let index = numeric_source(&span[1], constants, direct(&span[1]))?;
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

fn update_array_product_at_drop(
    facts: RegionFacts<'_>,
    nodes: &[UseNode],
    inputs: &[usize],
    initialized: &[bool],
    drop_node: usize,
) -> Option<(usize, UpdateDelta)> {
    let code = facts.code;
    let drop = nodes[drop_node].pc;
    let start = drop.checked_sub(11)?;
    let span = code.get(start..=drop)?;
    if !matches!(span[2], Instruction::GetArrayEl3)
        || !matches!(span[6], Instruction::GetArrayEl)
        || !matches!(span[7], Instruction::Mul)
        || !matches!(span[8], Instruction::Add)
        || !matches!(span[9], Instruction::Insert3)
        || !matches!(span[10], Instruction::PutArrayEl)
        || !matches!(span[11], Instruction::Drop)
    {
        return None;
    }
    let product_node = drop_node.checked_sub(4)?;
    if nodes[product_node].pc != start + 7 {
        return None;
    }
    let (product, members) = array_product_source(facts, nodes, inputs, initialized, product_node)?;
    if members.map(|node| nodes[node].pc) != [start + 4, start + 5, start + 6, start + 3, start + 7]
        || members.iter().any(|&node| nodes[node].uses != 1)
    {
        return None;
    }
    Some((start, UpdateDelta::ArrayProduct(product)))
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
