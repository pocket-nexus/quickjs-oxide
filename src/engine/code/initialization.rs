//! Conservative definite-initialization facts for numeric-region selection.
//! Only ordinary CFG edges carry facts. Exceptional and resume entries start
//! with no established lexical initialization, never a guessed runtime type.

use super::bytecode::Instruction;
use super::instruction::ControlEffect;

/// Entry state at each instruction that starts a basic block. Non-entry and
/// unreachable positions are `None`; callers treat those as unknown.
pub(crate) fn definite_initialization_entries(
    code: &[Instruction],
    local_count: usize,
) -> Vec<Option<Vec<bool>>> {
    let mut result = vec![None; code.len()];
    if code.is_empty() || local_count == 0 {
        return result;
    }
    let mut starts = vec![false; code.len()];
    starts[0] = true;
    for (pc, instruction) in code.iter().enumerate() {
        let control = instruction.control_effect();
        if let Some(target) = control.target()
            && let Some(start) = starts.get_mut(target as usize)
        {
            *start = true;
        }
        if control.ends_block()
            && let Some(start) = starts.get_mut(pc + 1)
        {
            *start = true;
        }
    }
    let begin: Vec<usize> = starts
        .iter()
        .enumerate()
        .filter_map(|(pc, &start)| start.then_some(pc))
        .collect();
    let mut block_at = vec![0usize; code.len()];
    for (block, &start) in begin.iter().enumerate() {
        let end = begin.get(block + 1).copied().unwrap_or(code.len());
        block_at[start..end].fill(block);
    }
    let count = begin.len();
    let mut normal_predecessors = vec![Vec::<usize>::new(); count];
    let mut successors = vec![Vec::<usize>::new(); count];
    let mut forced_unknown = vec![false; count];
    forced_unknown[0] = true;
    for (block, next_blocks) in successors.iter_mut().enumerate() {
        let end = begin.get(block + 1).copied().unwrap_or(code.len());
        let control = code[end - 1].control_effect();
        let fallthrough = (end < code.len()).then(|| block_at[end]);
        let target = control
            .target()
            .and_then(|pc| block_at.get(pc as usize).copied());
        let mut ordinary = |next: Option<usize>| {
            if let Some(next) = next {
                normal_predecessors[next].push(block);
                next_blocks.push(next);
            }
        };
        match control {
            ControlEffect::Next => ordinary(fallthrough),
            ControlEffect::Jump(_) => ordinary(target),
            ControlEffect::Branch(_) => {
                ordinary(target);
                ordinary(fallthrough);
            }
            ControlEffect::Catch(_) | ControlEffect::Gosub(_) => {
                for next in [target, fallthrough].into_iter().flatten() {
                    forced_unknown[next] = true;
                    next_blocks.push(next);
                }
            }
            ControlEffect::Suspend => {
                if let Some(next) = fallthrough {
                    forced_unknown[next] = true;
                    next_blocks.push(next);
                }
            }
            ControlEffect::Ret
            | ControlEffect::Return
            | ControlEffect::TailCall
            | ControlEffect::Throw => {}
        }
    }
    let mut reachable = vec![false; count];
    reachable[0] = true;
    let mut work = vec![0];
    while let Some(block) = work.pop() {
        for &next in &successors[block] {
            if !reachable[next] {
                reachable[next] = true;
                work.push(next);
            }
        }
    }
    // Start non-entry blocks at the lattice top. Intersection can only clear
    // bits, so loop backedges converge without assuming their first visit is
    // uninitialized.
    let mut inbound = vec![vec![true; local_count]; count];
    let mut outbound = inbound.clone();
    let mut changed = true;
    while changed {
        changed = false;
        for block in 0..count {
            if !reachable[block] {
                continue;
            }
            let mut next = vec![!forced_unknown[block]; local_count];
            for &predecessor in &normal_predecessors[block] {
                if reachable[predecessor] {
                    for (bit, &from) in next.iter_mut().zip(&outbound[predecessor]) {
                        *bit &= from;
                    }
                }
            }
            if next != inbound[block] {
                inbound[block] = next.clone();
                changed = true;
            }
            let end = begin.get(block + 1).copied().unwrap_or(code.len());
            for instruction in &code[begin[block]..end] {
                match instruction {
                    Instruction::InitializeLocal(index) => {
                        if let Some(bit) = next.get_mut(*index as usize) {
                            *bit = true;
                        }
                    }
                    Instruction::SetLocalUninitialized(index) => {
                        if let Some(bit) = next.get_mut(*index as usize) {
                            *bit = false;
                        }
                    }
                    _ => {}
                }
            }
            if next != outbound[block] {
                outbound[block] = next;
                changed = true;
            }
        }
    }
    for (block, &pc) in begin.iter().enumerate() {
        if reachable[block] {
            result[pc] = Some(inbound[block].clone());
        }
    }
    result
}
