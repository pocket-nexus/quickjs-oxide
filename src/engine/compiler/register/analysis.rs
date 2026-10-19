//! Control-flow facts the lowering needs before it emits anything: the static
//! operand-stack depth at every reachable instruction, the catch regions
//! active there, and which instructions are join points.
//!
//! This is `compute_stack_size` (`code/bytecode.rs`) restricted to the
//! instructions the lowering supports, keeping per-PC results instead of only
//! the maximum.

use std::collections::VecDeque;

use super::LowerError;
use crate::engine::code::bytecode::Instruction;

/// One `Catch` instruction: the stack depth of its marker and its handler;
/// or one iterator record (`ForOfStart` and friends):
/// the depth of its three slots, and no handler of its own (the lowering
/// adds one that closes the iterator and rethrows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CatchRegion {
    pub(super) marker_depth: u32,
    pub(super) target: usize,
    pub(super) iterator: bool,
    /// The instruction that opens the region; the regions active there
    /// enclose it.
    pub(super) opened_at: usize,
    /// The innermost region active at `opened_at`, or [`NO_REGION`].
    pub(super) parent: u32,
    /// The number of regions that enclose this one.
    pub(super) level: u32,
}

/// No active region.
pub(super) const NO_REGION: u32 = u32::MAX;

#[derive(Default)]
pub(super) struct Analysis {
    /// Operand-stack depth before each instruction; `None` if unreachable.
    pub(super) depth: Vec<Option<u32>>,
    /// The innermost catch region active before each instruction, as an
    /// index into `catches`, or [`NO_REGION`]; the rest follow its
    /// `parent` chain. A region is opened at one instruction only, so the
    /// chain from a region is the same wherever it is active.
    pub(super) regions: Vec<u32>,
    /// Jump, handler and `Gosub` targets, and the instruction after each
    /// `Gosub` (where `Ret` returns).
    pub(super) label: Vec<bool>,
    pub(super) catches: Vec<CatchRegion>,
    pub(super) max_depth: u32,
}

/// The working buffers of [`analyze_into`], kept between functions.
#[derive(Default)]
pub(super) struct Scratch {
    catch_ids: Vec<u32>,
    worklist: VecDeque<(usize, u32, u32)>,
}

#[cfg(test)]
pub(super) fn analyze(code: &[Instruction]) -> Result<Analysis, LowerError> {
    let mut analysis = Analysis::default();
    analyze_into(code, &mut analysis, &mut Scratch::default())?;
    Ok(analysis)
}

/// Analyzes `code` into `analysis`, reusing its buffers and `scratch`'s.
pub(super) fn analyze_into(
    code: &[Instruction],
    analysis: &mut Analysis,
    scratch: &mut Scratch,
) -> Result<(), LowerError> {
    let len = code.len();
    analysis.depth.clear();
    analysis.depth.resize(len, None);
    analysis.regions.clear();
    analysis.regions.resize(len, NO_REGION);
    analysis.label.clear();
    analysis.label.resize(len, false);
    analysis.catches.clear();
    analysis.max_depth = 0;
    let catch_ids = &mut scratch.catch_ids;
    catch_ids.clear();
    catch_ids.resize(len, u32::MAX);
    let internal = |message: &str| LowerError::Internal(message.to_owned());
    let worklist = &mut scratch.worklist;
    worklist.clear();
    worklist.push_back((0_usize, 0_u32, NO_REGION));
    let target = |t: u32| -> Result<usize, LowerError> {
        let t = t as usize;
        if t < len {
            Ok(t)
        } else {
            Err(internal("jump target out of range"))
        }
    };
    while let Some((pc, depth, mut region)) = worklist.pop_front() {
        let Some(instruction) = code.get(pc) else {
            return Err(internal("control falls off the end of the code"));
        };
        if let Some(seen) = analysis.depth[pc] {
            if seen != depth {
                return Err(internal("inconsistent stack depth at a join"));
            }
            continue;
        }
        analysis.depth[pc] = Some(depth);
        analysis.regions[pc] = region;
        analysis.max_depth = analysis.max_depth.max(depth);

        let (popped, pushed) = instruction.stack_effect();
        let remaining = depth
            .checked_sub(popped as u32)
            .ok_or_else(|| internal("stack underflow"))?;
        let mut next = remaining + pushed as u32;
        // The region a cleanup instruction closes.
        let closed = |catches: &[CatchRegion], region: u32, message: &str| {
            catches
                .get(region as usize)
                .copied()
                .ok_or_else(|| internal(message))
        };
        match instruction {
            Instruction::DropCatch => {
                region = closed(&analysis.catches, region, "catch region underflow")?.parent;
            }
            Instruction::NipCatch => {
                let catch = closed(&analysis.catches, region, "catch region underflow")?;
                if catch.iterator {
                    return Err(internal("NipCatch over an iterator record"));
                }
                next = catch.marker_depth + 1;
                region = catch.parent;
            }
            Instruction::ForOfStart | Instruction::ForAwaitOfStart => {
                if catch_ids[pc] == u32::MAX {
                    catch_ids[pc] = analysis.catches.len() as u32;
                    let catch = open(&analysis.catches, region, remaining, usize::MAX, true, pc);
                    analysis.catches.push(catch);
                }
                region = catch_ids[pc];
            }
            Instruction::IteratorClose
            | Instruction::IteratorClosePreserve
            | Instruction::IteratorDropPreserve
            | Instruction::IteratorDetachPreserve => {
                let catch = closed(&analysis.catches, region, "iterator region underflow")?;
                if !catch.iterator {
                    return Err(internal("iterator cleanup over a catch region"));
                }
                match instruction {
                    Instruction::IteratorClosePreserve | Instruction::IteratorDropPreserve => {
                        next = catch.marker_depth + 1;
                    }
                    Instruction::IteratorDetachPreserve => next = catch.marker_depth + 2,
                    _ => {}
                }
                region = catch.parent;
            }
            _ => {}
        }
        analysis.max_depth = analysis.max_depth.max(next);
        match *instruction {
            Instruction::Return
            | Instruction::ReturnDerived(_)
            | Instruction::Throw
            | Instruction::Ret
            | Instruction::ThrowReadOnly(_)
            | Instruction::ThrowRedeclaration(_)
            | Instruction::ThrowDeleteSuper
            | Instruction::ThrowIteratorMissingThrow => {}
            Instruction::Goto(t) => {
                let t = target(t)?;
                analysis.label[t] = true;
                worklist.push_back((t, next, region));
            }
            Instruction::IfFalse(t) | Instruction::IfTrue(t) => {
                let t = target(t)?;
                analysis.label[t] = true;
                worklist.push_back((t, next, region));
                worklist.push_back((pc + 1, next, region));
            }
            Instruction::Catch(t) => {
                let t = target(t)?;
                analysis.label[t] = true;
                if catch_ids[pc] == u32::MAX {
                    catch_ids[pc] = analysis.catches.len() as u32;
                    let catch = open(&analysis.catches, region, depth, t, false, pc);
                    analysis.catches.push(catch);
                }
                worklist.push_back((t, next, region));
                worklist.push_back((pc + 1, next, catch_ids[pc]));
            }
            Instruction::Gosub(t) => {
                let t = target(t)?;
                analysis.label[t] = true;
                if pc + 1 < len {
                    analysis.label[pc + 1] = true;
                }
                analysis.max_depth = analysis.max_depth.max(depth + 1);
                worklist.push_back((t, depth + 1, region));
                worklist.push_back((pc + 1, next, region));
            }
            _ => worklist.push_back((pc + 1, next, region)),
        }
    }
    Ok(())
}

/// A region opened inside `parent`.
fn open(
    catches: &[CatchRegion],
    parent: u32,
    marker_depth: u32,
    target: usize,
    iterator: bool,
    opened_at: usize,
) -> CatchRegion {
    CatchRegion {
        marker_depth,
        target,
        iterator,
        opened_at,
        parent,
        level: catches
            .get(parent as usize)
            .map_or(0, |parent| parent.level + 1),
    }
}

impl Analysis {
    /// The regions active before `pc`, innermost first.
    pub(super) fn chain(&self, pc: usize) -> impl Iterator<Item = (u32, &CatchRegion)> {
        let mut region = self.regions.get(pc).copied().unwrap_or(NO_REGION);
        std::iter::from_fn(move || {
            let catch = self.catches.get(region as usize)?;
            let id = region;
            region = catch.parent;
            Some((id, catch))
        })
    }
}
