//! Control-flow facts the lowering needs before it emits anything: the static
//! operand-stack depth at every reachable instruction, the catch regions
//! active there, and which instructions are join points.
//!
//! This is `compute_stack_size` (`code/bytecode.rs`) restricted to the phase 2
//! surface, keeping per-PC results instead of only the maximum.

use std::collections::VecDeque;

use super::LowerError;
use crate::engine::code::bytecode::Instruction;

/// One `Catch` instruction: the stack depth of its marker and its handler;
/// or one iterator record (`ForOfStart` and friends, phase 4 section 4.2):
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
}

pub(super) struct Analysis {
    /// Operand-stack depth before each instruction; `None` if unreachable.
    pub(super) depth: Vec<Option<u32>>,
    /// Active catch regions before each instruction, innermost last, as
    /// indices into `catches`.
    pub(super) regions: Vec<Vec<u32>>,
    /// Jump, handler and `Gosub` targets, and the instruction after each
    /// `Gosub` (where `Ret` returns).
    pub(super) label: Vec<bool>,
    pub(super) catches: Vec<CatchRegion>,
    pub(super) max_depth: u32,
}

pub(super) fn analyze(code: &[Instruction]) -> Result<Analysis, LowerError> {
    let len = code.len();
    let mut analysis = Analysis {
        depth: vec![None; len],
        regions: vec![Vec::new(); len],
        label: vec![false; len],
        catches: Vec::new(),
        max_depth: 0,
    };
    let mut catch_ids = vec![u32::MAX; len];
    let internal = |message: &str| LowerError::Internal(message.to_owned());
    let mut worklist = VecDeque::from([(0_usize, 0_u32, Vec::<u32>::new())]);
    let target = |t: u32| -> Result<usize, LowerError> {
        let t = t as usize;
        if t < len {
            Ok(t)
        } else {
            Err(internal("jump target out of range"))
        }
    };
    while let Some((pc, depth, mut regions)) = worklist.pop_front() {
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
        analysis.regions[pc] = regions.clone();
        analysis.max_depth = analysis.max_depth.max(depth);

        let (popped, pushed) = instruction.stack_effect();
        let remaining = depth
            .checked_sub(popped as u32)
            .ok_or_else(|| internal("stack underflow"))?;
        let mut next = remaining + pushed as u32;
        match instruction {
            Instruction::DropCatch => {
                regions
                    .pop()
                    .ok_or_else(|| internal("catch region underflow"))?;
            }
            Instruction::NipCatch => {
                let region = regions
                    .pop()
                    .ok_or_else(|| internal("catch region underflow"))?;
                if analysis.catches[region as usize].iterator {
                    return Err(internal("NipCatch over an iterator record"));
                }
                next = analysis.catches[region as usize].marker_depth + 1;
            }
            Instruction::ForOfStart | Instruction::ForAwaitOfStart => {
                if catch_ids[pc] == u32::MAX {
                    catch_ids[pc] = analysis.catches.len() as u32;
                    analysis.catches.push(CatchRegion {
                        marker_depth: remaining,
                        target: usize::MAX,
                        iterator: true,
                        opened_at: pc,
                    });
                }
                regions.push(catch_ids[pc]);
            }
            Instruction::IteratorClose
            | Instruction::IteratorClosePreserve
            | Instruction::IteratorDropPreserve
            | Instruction::IteratorDetachPreserve => {
                let region = regions
                    .pop()
                    .ok_or_else(|| internal("iterator region underflow"))?;
                let region = analysis.catches[region as usize];
                if !region.iterator {
                    return Err(internal("iterator cleanup over a catch region"));
                }
                match instruction {
                    Instruction::IteratorClosePreserve | Instruction::IteratorDropPreserve => {
                        next = region.marker_depth + 1;
                    }
                    Instruction::IteratorDetachPreserve => next = region.marker_depth + 2,
                    _ => {}
                }
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
                worklist.push_back((t, next, regions));
            }
            Instruction::IfFalse(t) | Instruction::IfTrue(t) => {
                let t = target(t)?;
                analysis.label[t] = true;
                worklist.push_back((t, next, regions.clone()));
                worklist.push_back((pc + 1, next, regions));
            }
            Instruction::Catch(t) => {
                let t = target(t)?;
                analysis.label[t] = true;
                if catch_ids[pc] == u32::MAX {
                    catch_ids[pc] = analysis.catches.len() as u32;
                    analysis.catches.push(CatchRegion {
                        marker_depth: depth,
                        target: t,
                        iterator: false,
                        opened_at: pc,
                    });
                }
                worklist.push_back((t, next, regions.clone()));
                regions.push(catch_ids[pc]);
                worklist.push_back((pc + 1, next, regions));
            }
            Instruction::Gosub(t) => {
                let t = target(t)?;
                analysis.label[t] = true;
                if pc + 1 < len {
                    analysis.label[pc + 1] = true;
                }
                analysis.max_depth = analysis.max_depth.max(depth + 1);
                worklist.push_back((t, depth + 1, regions.clone()));
                worklist.push_back((pc + 1, next, regions));
            }
            _ => worklist.push_back((pc + 1, next, regions)),
        }
    }
    Ok(analysis)
}
