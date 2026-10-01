//! Conservative definite-initialization facts for numeric-region selection.
//! Only ordinary CFG edges carry facts. Exceptional and resume entries start
//! with no established lexical initialization, never a guessed runtime type.

use super::bytecode::Instruction;
use super::instruction::ControlEffect;
use std::collections::VecDeque;

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
    // Transfer effects are independent of predecessor state. Scan each block
    // once, then converge only blocks whose incoming facts may have changed.
    let words = local_count.div_ceil(64);
    let mut sets = vec![vec![0u64; words]; count];
    let mut clears = sets.clone();
    for block in 0..count {
        let end = begin.get(block + 1).copied().unwrap_or(code.len());
        for instruction in &code[begin[block]..end] {
            let (index, initialized) = match instruction {
                Instruction::InitializeLocal(index) => (*index as usize, true),
                Instruction::SetLocalUninitialized(index) => (*index as usize, false),
                _ => continue,
            };
            if index >= local_count {
                continue;
            }
            let word = index / 64;
            let bit = 1u64 << (index % 64);
            if initialized {
                sets[block][word] |= bit;
                clears[block][word] &= !bit;
            } else {
                clears[block][word] |= bit;
                sets[block][word] &= !bit;
            }
        }
    }
    // Start non-entry blocks at the lattice top. Intersection can only clear
    // bits, so loop backedges converge without guessing initialization.
    let mut inbound = vec![vec![u64::MAX; words]; count];
    let mut outbound = inbound.clone();
    let mut queue = VecDeque::new();
    let mut queued = vec![false; count];
    for (block, &is_reachable) in reachable.iter().enumerate() {
        if is_reachable {
            queue.push_back(block);
            queued[block] = true;
        }
    }
    while let Some(block) = queue.pop_front() {
        queued[block] = false;
        let mut next = vec![if forced_unknown[block] { 0 } else { u64::MAX }; words];
        for &predecessor in &normal_predecessors[block] {
            if reachable[predecessor] {
                for (word, &from) in next.iter_mut().zip(&outbound[predecessor]) {
                    *word &= from;
                }
            }
        }
        inbound[block] = next.clone();
        for (word, (&set, &clear)) in next.iter_mut().zip(sets[block].iter().zip(&clears[block])) {
            *word = (*word & !clear) | set;
        }
        if next != outbound[block] {
            outbound[block] = next;
            for &successor in &successors[block] {
                if reachable[successor] && !queued[successor] {
                    queue.push_back(successor);
                    queued[successor] = true;
                }
            }
        }
    }
    for (block, &pc) in begin.iter().enumerate() {
        if reachable[block] {
            result[pc] = Some(
                (0..local_count)
                    .map(|index| inbound[block][index / 64] & (1u64 << (index % 64)) != 0)
                    .collect(),
            );
        }
    }
    result
}
