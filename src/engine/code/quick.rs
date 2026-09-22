//! Read-only B1a execution words, indexed by unchanged canonical PCs.
//!
//! This module is test-only until B1b integrates the verified publisher. Its
//! constructors remain private: neither a wire reader nor a draft can supply
//! an executable word. Canonical verification is a prerequisite; projection
//! validation authenticates the translation, not the source bytecode itself.
//! No VM, heap, runtime owner, feedback state, or executable handler lives here.

use super::bytecode::Instruction;
use super::instruction::{
    ControlEffect, JsExceptionEffect, Operand, OperandContract, PotentialEffects, StackEffect,
    StackStateEffect,
};
use std::collections::TryReserveError;
use std::rc::Rc;

mod translate;
use translate::translate_instruction;

#[cfg(test)]
mod tests;

/// Integer layout, independent of the host's byte order:
/// tag: 0..=7, flags: 8..=15, aux: 16..=31, operand: 32..=63.
/// Flags and aux are reserved and must both be zero in this read-only version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
struct QuickOp(u64);

const _: () = assert!(std::mem::size_of::<QuickOp>() == 8);
const RESERVED_MASK: u64 = 0x0000_0000_ffff_ff00;

/// These discriminants are private execution metadata, never BC5 wire tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
enum QuickTag {
    GenericCanonical = 0,
    Nop = 1,
    PushI32 = 2,
    Undefined = 3,
    Null = 4,
    Bool = 5,
    Goto = 6,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecodedOp {
    GenericCanonical,
    Nop,
    PushI32(i32),
    Undefined,
    Null,
    Bool(bool),
    Goto(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecodeError {
    ReservedBits,
    UnknownTag(u8),
    UnexpectedOperand,
    InvalidBoolean(u32),
}

impl QuickOp {
    const fn pack(tag: QuickTag, operand: u32) -> Self {
        Self((tag as u64) | ((operand as u64) << 32))
    }

    fn decode(self) -> Result<DecodedOp, DecodeError> {
        if self.0 & RESERVED_MASK != 0 {
            return Err(DecodeError::ReservedBits);
        }
        // Both conversions are bounded by the explicit mask/shift, never
        // truncation of a source operand that did not fit the word.
        let tag = u8::try_from(self.0 & 0xff).expect("masked tag fits u8");
        let operand = u32::try_from(self.0 >> 32).expect("shifted operand fits u32");
        let tag = match tag {
            0 => QuickTag::GenericCanonical,
            1 => QuickTag::Nop,
            2 => QuickTag::PushI32,
            3 => QuickTag::Undefined,
            4 => QuickTag::Null,
            5 => QuickTag::Bool,
            6 => QuickTag::Goto,
            unknown => return Err(DecodeError::UnknownTag(unknown)),
        };
        match tag {
            QuickTag::PushI32 => Ok(DecodedOp::PushI32(i32::from_le_bytes(
                operand.to_le_bytes(),
            ))),
            QuickTag::Bool => match operand {
                0 => Ok(DecodedOp::Bool(false)),
                1 => Ok(DecodedOp::Bool(true)),
                invalid => Err(DecodeError::InvalidBoolean(invalid)),
            },
            QuickTag::Goto => Ok(DecodedOp::Goto(operand)),
            QuickTag::GenericCanonical | QuickTag::Nop | QuickTag::Undefined | QuickTag::Null => {
                if operand != 0 {
                    return Err(DecodeError::UnexpectedOperand);
                }
                Ok(match tag {
                    QuickTag::GenericCanonical => DecodedOp::GenericCanonical,
                    QuickTag::Nop => DecodedOp::Nop,
                    QuickTag::Undefined => DecodedOp::Undefined,
                    QuickTag::Null => DecodedOp::Null,
                    QuickTag::PushI32 | QuickTag::Bool | QuickTag::Goto => unreachable!(),
                })
            }
        }
    }
}

/// A certified all-cold function owns no word buffer. Words retains one word
/// for *every* canonical PC, including Generic opcodes and fusion interiors.
/// There is no Default/absent state and no fallback on malformed Words.
/// The private representation exposes only a shared slice, never Rc or Vec
/// mutation; cloning a projection shares its buffer.
#[derive(Clone, Debug)]
enum QuickProgram {
    CanonicalOnly,
    Words(Rc<Vec<QuickOp>>),
}

#[derive(Debug)]
enum BuildError {
    Allocation(TryReserveError),
    InvalidProjection(ValidationError),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValidationError {
    Length { canonical: usize, quick: usize },
    InvalidWord { pc: usize, reason: DecodeError },
    CanonicalMismatch { pc: usize },
    ContractMismatch { pc: usize },
    HotInstructionInCanonicalOnly { pc: usize },
}

impl QuickProgram {
    /// Call only after canonical code and metadata verification. Two bounded
    /// scans plus projection validation are O(N); auxiliary scratch is O(1).
    fn build(code: &[Instruction]) -> Result<Self, BuildError> {
        if code.iter().all(|instruction| {
            translate_instruction(instruction) == QuickOp::pack(QuickTag::GenericCanonical, 0)
        }) {
            return Ok(Self::CanonicalOnly);
        }

        let mut words = reserve_words(code.len())?;
        // Every push fits the reserved buffer. There is no shrink, boxed-slice
        // conversion, side table, or second large allocation when sharing it.
        for instruction in code {
            words.push(translate_instruction(instruction));
        }
        validate_words(code, &words).map_err(BuildError::InvalidProjection)?;
        // This small Rc allocation follows std's global allocator OOM policy;
        // only the large Vec reservation above is recoverable via Result.
        Ok(Self::Words(Rc::new(words)))
    }

    fn words(&self) -> Option<&[QuickOp]> {
        match self {
            Self::CanonicalOnly => None,
            Self::Words(words) => Some(words.as_slice()),
        }
    }

    fn validate(&self, code: &[Instruction]) -> Result<(), ValidationError> {
        match self {
            Self::CanonicalOnly => {
                for (pc, instruction) in code.iter().enumerate() {
                    if translate_instruction(instruction)
                        != QuickOp::pack(QuickTag::GenericCanonical, 0)
                    {
                        return Err(ValidationError::HotInstructionInCanonicalOnly { pc });
                    }
                }
                Ok(())
            }
            Self::Words(words) => validate_words(code, words),
        }
    }
}

fn reserve_words(len: usize) -> Result<Vec<QuickOp>, BuildError> {
    let mut words = Vec::new();
    words
        .try_reserve_exact(len)
        .map_err(BuildError::Allocation)?;
    Ok(words)
}

fn validate_words(code: &[Instruction], words: &[QuickOp]) -> Result<(), ValidationError> {
    if words.len() != code.len() {
        return Err(ValidationError::Length {
            canonical: code.len(),
            quick: words.len(),
        });
    }
    for (pc, (instruction, word)) in code.iter().zip(words).enumerate() {
        let decoded = word
            .decode()
            .map_err(|reason| ValidationError::InvalidWord { pc, reason })?;
        // This enforces the single allowed read-only tag and all argument bits
        // at this exact canonical PC. Generic carries no lossy PC operand.
        if *word != translate_instruction(instruction) {
            return Err(ValidationError::CanonicalMismatch { pc });
        }
        if !contracts_match(decoded, instruction) {
            return Err(ValidationError::ContractMismatch { pc });
        }
    }
    Ok(())
}

/// Independently check the narrow hot-handler requirements against the four
/// authoritative canonical contracts; do not recreate an Instruction or use
/// profiling-only Instruction::info(). Generic executes its canonical source.
fn contracts_match(decoded: DecodedOp, instruction: &Instruction) -> bool {
    let (pushed, control, operand) = match decoded {
        DecodedOp::GenericCanonical => return true,
        DecodedOp::Nop => (0, ControlEffect::Next, None),
        DecodedOp::PushI32(value) => (1, ControlEffect::Next, Some(Operand::Integer(value))),
        DecodedOp::Undefined | DecodedOp::Null | DecodedOp::Bool(_) => {
            (1, ControlEffect::Next, None)
        }
        DecodedOp::Goto(target) => (
            0,
            ControlEffect::Jump(target),
            Some(Operand::Target(target)),
        ),
    };
    // The canonical control-flow contract conservatively permits JavaScript
    // completion for Goto. Preserve that classification even though its
    // present successful run body only updates the next PC.
    let javascript_exception = if matches!(decoded, DecodedOp::Goto(_)) {
        JsExceptionEffect::MayThrow
    } else {
        JsExceptionEffect::None
    };
    instruction.stack_contract()
        == StackEffect {
            popped: 0,
            pushed,
            state: StackStateEffect::Ordinary,
        }
        && instruction.control_effect() == control
        && instruction.operand_contract() == OperandContract([operand, None, None])
        && instruction.potential_effects()
            == PotentialEffects {
                javascript_exception,
                may_call_js: false,
                may_allocate: false,
            }
}
