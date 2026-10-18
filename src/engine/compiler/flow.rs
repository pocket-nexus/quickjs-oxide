//! Stack-size computation and block entries used during lowering.
//!
//! `compute_stack_size` walks reachable normal, exception and resume states.
//! This adapter never replaces it with a syntactic stack-height estimate or a
//! budget-limited dataflow result.

use crate::engine::api::error::{Error, ErrorKind};
use crate::engine::code::bytecode::{Instruction, compute_stack_size};
use crate::engine::compiler::MAX_BYTECODE_STACK;

pub(super) fn lowered_max_stack(code: &[Instruction]) -> Result<u16, Error> {
    compute_stack_size(code, MAX_BYTECODE_STACK as u16)
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
/// locals, or a valid resume shape.
pub(super) fn block_entries(code: &[Instruction]) -> Vec<bool> {
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
