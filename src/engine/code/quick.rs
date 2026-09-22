//! Read-only execution words, indexed by unchanged canonical PCs.
//!
//! Built eagerly only under the internal `oxide_quick_projection` experiment
//! (or in unit tests). Neither a wire reader nor a draft can supply executable
//! words; the publisher rebuilds them from the exact authenticated payload. Canonical verification is a prerequisite; projection
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
pub(crate) mod reservation_failure;

#[cfg(test)]
mod tests;

/// Integer layout, independent of the host's byte order:
/// tag: 0..=7, flags: 8..=15, aux: 16..=31, operand: 32..=63.
/// Flags and aux are reserved and must both be zero in this read-only version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub(crate) struct QuickOp(u64);

const _: () = assert!(std::mem::size_of::<QuickOp>() == 8);
const RESERVED_MASK: u64 = 0x0000_0000_ffff_ff00;

// Define codec discriminants and execution match constants together. Reading
// a certified word must not convert u8 -> enum -> DecodedOp before dispatch.
macro_rules! quick_tags {
    ($($variant:ident => $constant:ident = $value:literal),+ $(,)?) => {
        /// Private execution metadata, never BC5 wire tags.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        #[repr(u8)]
        enum QuickTag {
            $($variant = $value,)+
        }

        #[cfg(any(test, oxide_quick_dispatch))]
        pub(crate) mod tag {
            $(pub(crate) const $constant: u8 = super::QuickTag::$variant as u8;)+
        }
    };
}

quick_tags! {
    GenericCanonical => GENERIC_CANONICAL = 0,
    Nop => NOP = 1,
    PushI32 => PUSH_I32 = 2,
    Undefined => UNDEFINED = 3,
    Null => NULL = 4,
    Bool => BOOL = 5,
    Goto => GOTO = 6,
    Add => ADD = 7,
    Sub => SUB = 8,
    Mul => MUL = 9,
    Div => DIV = 10,
    Mod => MOD = 11,
    Pow => POW = 12,
    Shl => SHL = 13,
    Sar => SAR = 14,
    Shr => SHR = 15,
    BitAnd => BIT_AND = 16,
    BitOr => BIT_OR = 17,
    BitXor => BIT_XOR = 18,
    Eq => EQ = 19,
    Neq => NEQ = 20,
    Lt => LT = 21,
    Lte => LTE = 22,
    Gt => GT = 23,
    Gte => GTE = 24,
    IfTrue => IF_TRUE = 25,
    IfFalse => IF_FALSE = 26,
    GetLocal => GET_LOCAL = 27,
    PutLocal => PUT_LOCAL = 28,
    SetLocal => SET_LOCAL = 29,
    GetArg => GET_ARG = 30,
    PutArg => PUT_ARG = 31,
    SetArg => SET_ARG = 32,
}

#[cfg(any(test, feature = "profiling"))]
pub(crate) const QUICK_TAG_COUNT: usize = QuickTag::SetArg as usize + 1;
const _: () = assert!(QuickTag::SetArg as usize + 1 == 33);

#[cfg(feature = "profiling")]
pub(crate) const QUICK_TAG_PROFILE_NAMES: [&str; QUICK_TAG_COUNT] = [
    "tag.GenericCanonical",
    "tag.Nop",
    "tag.PushI32",
    "tag.Undefined",
    "tag.Null",
    "tag.Bool",
    "tag.Goto",
    "tag.Add",
    "tag.Sub",
    "tag.Mul",
    "tag.Div",
    "tag.Mod",
    "tag.Pow",
    "tag.Shl",
    "tag.Sar",
    "tag.Shr",
    "tag.BitAnd",
    "tag.BitOr",
    "tag.BitXor",
    "tag.Eq",
    "tag.Neq",
    "tag.Lt",
    "tag.Lte",
    "tag.Gt",
    "tag.Gte",
    "tag.IfTrue",
    "tag.IfFalse",
    "tag.GetLocal",
    "tag.PutLocal",
    "tag.SetLocal",
    "tag.GetArg",
    "tag.PutArg",
    "tag.SetArg",
];

#[cfg(feature = "profiling")]
pub(crate) const QUICK_TAG_MEMORY_NAMES: [&str; QUICK_TAG_COUNT] = [
    "bytecode_quick_tag_generic",
    "bytecode_quick_tag_nop",
    "bytecode_quick_tag_push_i32",
    "bytecode_quick_tag_undefined",
    "bytecode_quick_tag_null",
    "bytecode_quick_tag_bool",
    "bytecode_quick_tag_goto",
    "bytecode_quick_tag_add",
    "bytecode_quick_tag_sub",
    "bytecode_quick_tag_mul",
    "bytecode_quick_tag_div",
    "bytecode_quick_tag_mod",
    "bytecode_quick_tag_pow",
    "bytecode_quick_tag_shl",
    "bytecode_quick_tag_sar",
    "bytecode_quick_tag_shr",
    "bytecode_quick_tag_bit_and",
    "bytecode_quick_tag_bit_or",
    "bytecode_quick_tag_bit_xor",
    "bytecode_quick_tag_eq",
    "bytecode_quick_tag_neq",
    "bytecode_quick_tag_lt",
    "bytecode_quick_tag_lte",
    "bytecode_quick_tag_gt",
    "bytecode_quick_tag_gte",
    "bytecode_quick_tag_if_true",
    "bytecode_quick_tag_if_false",
    "bytecode_quick_tag_get_local",
    "bytecode_quick_tag_put_local",
    "bytecode_quick_tag_set_local",
    "bytecode_quick_tag_get_arg",
    "bytecode_quick_tag_put_arg",
    "bytecode_quick_tag_set_arg",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodedOp {
    GenericCanonical,
    Nop,
    PushI32(i32),
    Undefined,
    Null,
    Bool(bool),
    Goto(u32),
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Shl,
    Sar,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    IfTrue(u32),
    IfFalse(u32),
    GetLocal(u16),
    PutLocal(u16),
    SetLocal(u16),
    GetArg(u16),
    PutArg(u16),
    SetArg(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DecodeError {
    ReservedBits,
    UnknownTag(u8),
    UnexpectedOperand,
    InvalidBoolean(u32),
    InvalidSlot(u32),
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
            7 => QuickTag::Add,
            8 => QuickTag::Sub,
            9 => QuickTag::Mul,
            10 => QuickTag::Div,
            11 => QuickTag::Mod,
            12 => QuickTag::Pow,
            13 => QuickTag::Shl,
            14 => QuickTag::Sar,
            15 => QuickTag::Shr,
            16 => QuickTag::BitAnd,
            17 => QuickTag::BitOr,
            18 => QuickTag::BitXor,
            19 => QuickTag::Eq,
            20 => QuickTag::Neq,
            21 => QuickTag::Lt,
            22 => QuickTag::Lte,
            23 => QuickTag::Gt,
            24 => QuickTag::Gte,
            25 => QuickTag::IfTrue,
            26 => QuickTag::IfFalse,
            27 => QuickTag::GetLocal,
            28 => QuickTag::PutLocal,
            29 => QuickTag::SetLocal,
            30 => QuickTag::GetArg,
            31 => QuickTag::PutArg,
            32 => QuickTag::SetArg,
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
            QuickTag::IfTrue => Ok(DecodedOp::IfTrue(operand)),
            QuickTag::IfFalse => Ok(DecodedOp::IfFalse(operand)),
            QuickTag::GetLocal => u16::try_from(operand)
                .map(DecodedOp::GetLocal)
                .map_err(|_| DecodeError::InvalidSlot(operand)),
            QuickTag::PutLocal => u16::try_from(operand)
                .map(DecodedOp::PutLocal)
                .map_err(|_| DecodeError::InvalidSlot(operand)),
            QuickTag::SetLocal => u16::try_from(operand)
                .map(DecodedOp::SetLocal)
                .map_err(|_| DecodeError::InvalidSlot(operand)),
            QuickTag::GetArg => u16::try_from(operand)
                .map(DecodedOp::GetArg)
                .map_err(|_| DecodeError::InvalidSlot(operand)),
            QuickTag::PutArg => u16::try_from(operand)
                .map(DecodedOp::PutArg)
                .map_err(|_| DecodeError::InvalidSlot(operand)),
            QuickTag::SetArg => u16::try_from(operand)
                .map(DecodedOp::SetArg)
                .map_err(|_| DecodeError::InvalidSlot(operand)),
            QuickTag::GenericCanonical
            | QuickTag::Nop
            | QuickTag::Undefined
            | QuickTag::Null
            | QuickTag::Add
            | QuickTag::Sub
            | QuickTag::Mul
            | QuickTag::Div
            | QuickTag::Mod
            | QuickTag::Pow
            | QuickTag::Shl
            | QuickTag::Sar
            | QuickTag::Shr
            | QuickTag::BitAnd
            | QuickTag::BitOr
            | QuickTag::BitXor
            | QuickTag::Eq
            | QuickTag::Neq
            | QuickTag::Lt
            | QuickTag::Lte
            | QuickTag::Gt
            | QuickTag::Gte => {
                if operand != 0 {
                    return Err(DecodeError::UnexpectedOperand);
                }
                Ok(match tag {
                    QuickTag::GenericCanonical => DecodedOp::GenericCanonical,
                    QuickTag::Nop => DecodedOp::Nop,
                    QuickTag::Undefined => DecodedOp::Undefined,
                    QuickTag::Null => DecodedOp::Null,
                    QuickTag::Add => DecodedOp::Add,
                    QuickTag::Sub => DecodedOp::Sub,
                    QuickTag::Mul => DecodedOp::Mul,
                    QuickTag::Div => DecodedOp::Div,
                    QuickTag::Mod => DecodedOp::Mod,
                    QuickTag::Pow => DecodedOp::Pow,
                    QuickTag::Shl => DecodedOp::Shl,
                    QuickTag::Sar => DecodedOp::Sar,
                    QuickTag::Shr => DecodedOp::Shr,
                    QuickTag::BitAnd => DecodedOp::BitAnd,
                    QuickTag::BitOr => DecodedOp::BitOr,
                    QuickTag::BitXor => DecodedOp::BitXor,
                    QuickTag::Eq => DecodedOp::Eq,
                    QuickTag::Neq => DecodedOp::Neq,
                    QuickTag::Lt => DecodedOp::Lt,
                    QuickTag::Lte => DecodedOp::Lte,
                    QuickTag::Gt => DecodedOp::Gt,
                    QuickTag::Gte => DecodedOp::Gte,
                    QuickTag::PushI32
                    | QuickTag::Bool
                    | QuickTag::Goto
                    | QuickTag::IfTrue
                    | QuickTag::IfFalse
                    | QuickTag::GetLocal
                    | QuickTag::PutLocal
                    | QuickTag::SetLocal
                    | QuickTag::GetArg
                    | QuickTag::PutArg
                    | QuickTag::SetArg => unreachable!(),
                })
            }
        }
    }
}

/// Execution access is confined to immutable words supplied by the certified
/// publisher. All bit/width/operand checks remain in decode + validate_words;
/// these accessors neither classify the tag nor reconstruct an Instruction.
#[cfg(any(test, oxide_quick_dispatch))]
impl QuickOp {
    #[inline(always)]
    pub(crate) const fn tag(self) -> u8 {
        (self.0 & 0xff) as u8
    }

    #[inline(always)]
    pub(crate) const fn operand(self) -> u32 {
        (self.0 >> 32) as u32
    }

    #[inline(always)]
    pub(crate) const fn i32_operand(self) -> i32 {
        i32::from_le_bytes(self.operand().to_le_bytes())
    }

    /// Call only for an authenticated local/argument tag. Publication has
    /// already rejected every operand above u16::MAX, before the word escapes.
    #[inline(always)]
    pub(crate) const fn slot(self) -> u16 {
        self.operand() as u16
    }

    /// Call only for BOOL, whose complete validator accepts exactly 0 or 1.
    #[inline(always)]
    pub(crate) const fn boolean(self) -> bool {
        self.operand() != 0
    }
}

/// A certified all-cold function owns no word buffer. Words retains one word
/// for *every* canonical PC, including Generic opcodes and fusion interiors.
/// There is no Default/absent state and no fallback on malformed Words.
/// The private representation exposes only a shared slice, never Rc or Vec
/// mutation; cloning a projection shares its buffer.
#[derive(Clone, Debug)]
pub(crate) struct QuickProgram(ProgramKind);

#[derive(Clone, Debug)]
enum ProgramKind {
    CanonicalOnly,
    Words(Rc<Vec<QuickOp>>),
}

#[derive(Debug)]
pub(crate) enum BuildError {
    Allocation(TryReserveError),
    InvalidProjection(ValidationError),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Allocation(error) => {
                write!(formatter, "QuickOp word reservation failed: {error}")
            }
            Self::InvalidProjection(error) => {
                write!(formatter, "QuickOp projection failed validation: {error:?}")
            }
        }
    }
}

/// Storage accounting identifies the shared Vec allocation and reports actual
/// capacity. Account capacity * 8 plus one Vec header and two Rc counters per
/// distinct buffer identity; allocator-specific overhead is not known here.
#[cfg(feature = "profiling")]
pub(crate) struct QuickStorage {
    pub(crate) buffer_identity: Option<usize>,
    pub(crate) word_len: usize,
    pub(crate) word_capacity: usize,
    pub(crate) tag_counts: [usize; QUICK_TAG_COUNT],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ValidationError {
    Length { canonical: usize, quick: usize },
    InvalidWord { pc: usize, reason: DecodeError },
    CanonicalMismatch { pc: usize },
    ContractMismatch { pc: usize },
    HotInstructionInCanonicalOnly { pc: usize },
}

impl QuickProgram {
    /// Borrow once at function entry; None is the certified all-cold mode.
    /// Sharing this slice does not allocate or increment the word buffer's Rc.
    /// QuickOp's private field/constructors keep raw words out of execution.
    #[cfg(any(test, oxide_quick_dispatch))]
    #[inline]
    pub(crate) fn execution_words(&self) -> Option<&[QuickOp]> {
        match &self.0 {
            ProgramKind::CanonicalOnly => None,
            ProgramKind::Words(words) => Some(words.as_slice()),
        }
    }

    /// Test-only counterpart of execution_words' function-level selection.
    #[cfg(test)]
    #[inline]
    pub(crate) fn has_words(&self) -> bool {
        matches!(&self.0, ProgramKind::Words(_))
    }

    /// Read one authenticated word at its unchanged canonical PC. A certified
    /// all-cold program and an absent PC select the caller's canonical path.
    /// Corrupt words are invariant failures, never a silent Generic fallback.
    #[cfg(test)]
    #[inline]
    pub(crate) fn operation(&self, pc: usize) -> Option<DecodedOp> {
        match &self.0 {
            ProgramKind::CanonicalOnly => None,
            ProgramKind::Words(words) => words.get(pc).map(|word| {
                word.decode()
                    .expect("published QuickOp word must remain authenticated")
            }),
        }
    }

    /// Call only after canonical code and metadata verification. Two bounded
    /// scans plus projection validation are O(N); auxiliary scratch is O(1).
    pub(crate) fn build_verified(code: &[Instruction]) -> Result<Self, BuildError> {
        #[cfg(feature = "profiling")]
        let _timer = crate::engine::api::profiling::PhaseTimer::start(
            crate::engine::api::profiling::CompilePhase::QuickProjection,
        );
        let program = if code.iter().all(|instruction| {
            translate_instruction(instruction) == QuickOp::pack(QuickTag::GenericCanonical, 0)
        }) {
            Self(ProgramKind::CanonicalOnly)
        } else {
            let mut words = reserve_words(code.len())?;
            // Every push fits the reserved buffer. There is no shrink, boxed-slice
            // conversion, side table, or second large allocation when sharing it.
            for instruction in code {
                words.push(translate_instruction(instruction));
            }
            // This small Rc allocation follows std's global allocator OOM policy;
            // only the large Vec reservation above is recoverable via Result.
            let program = Self(ProgramKind::Words(Rc::new(words)));
            program
                .validate(code)
                .map_err(BuildError::InvalidProjection)?;
            program
        };
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_quick_projection(&program, code.len());
        Ok(program)
    }

    #[cfg(test)]
    fn words(&self) -> Option<&[QuickOp]> {
        match &self.0 {
            ProgramKind::CanonicalOnly => None,
            ProgramKind::Words(words) => Some(words.as_slice()),
        }
    }

    pub(crate) fn validate(&self, code: &[Instruction]) -> Result<(), ValidationError> {
        match &self.0 {
            ProgramKind::CanonicalOnly => {
                for (pc, instruction) in code.iter().enumerate() {
                    if translate_instruction(instruction)
                        != QuickOp::pack(QuickTag::GenericCanonical, 0)
                    {
                        return Err(ValidationError::HotInstructionInCanonicalOnly { pc });
                    }
                }
                Ok(())
            }
            ProgramKind::Words(words) => validate_words(code, words),
        }
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn storage(&self) -> QuickStorage {
        let mut storage = QuickStorage {
            buffer_identity: None,
            word_len: 0,
            word_capacity: 0,
            tag_counts: [0; QUICK_TAG_COUNT],
        };
        if let ProgramKind::Words(words) = &self.0 {
            storage.buffer_identity = Some(Rc::as_ptr(words) as usize);
            storage.word_len = words.len();
            storage.word_capacity = words.capacity();
            for word in words.iter() {
                let tag = usize::try_from(word.0 & 0xff).expect("authenticated tag fits usize");
                storage.tag_counts[tag] += 1;
            }
        }
        storage
    }

    #[cfg(test)]
    pub(crate) fn shares_words_with(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (ProgramKind::Words(left), ProgramKind::Words(right)) => Rc::ptr_eq(left, right),
            (ProgramKind::CanonicalOnly, ProgramKind::CanonicalOnly) => true,
            _ => false,
        }
    }
}

fn reserve_words(len: usize) -> Result<Vec<QuickOp>, BuildError> {
    #[cfg(test)]
    if reservation_failure::should_fail() {
        let error = Vec::<QuickOp>::new()
            .try_reserve_exact(usize::MAX)
            .unwrap_err();
        return Err(BuildError::Allocation(error));
    }
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
    let (popped, pushed, control, operand) = match decoded {
        DecodedOp::GenericCanonical => return true,
        DecodedOp::Nop => (0, 0, ControlEffect::Next, None),
        DecodedOp::PushI32(value) => (0, 1, ControlEffect::Next, Some(Operand::Integer(value))),
        DecodedOp::Undefined | DecodedOp::Null | DecodedOp::Bool(_) => {
            (0, 1, ControlEffect::Next, None)
        }
        DecodedOp::Goto(target) => (
            0,
            0,
            ControlEffect::Jump(target),
            Some(Operand::Target(target)),
        ),
        DecodedOp::IfTrue(target) | DecodedOp::IfFalse(target) => (
            1,
            0,
            ControlEffect::Branch(target),
            Some(Operand::Target(target)),
        ),
        DecodedOp::GetLocal(index) => (0, 1, ControlEffect::Next, Some(Operand::Local(index))),
        DecodedOp::PutLocal(index) => (1, 0, ControlEffect::Next, Some(Operand::Local(index))),
        DecodedOp::SetLocal(index) => (1, 1, ControlEffect::Next, Some(Operand::Local(index))),
        DecodedOp::GetArg(index) => (0, 1, ControlEffect::Next, Some(Operand::Argument(index))),
        DecodedOp::PutArg(index) => (1, 0, ControlEffect::Next, Some(Operand::Argument(index))),
        DecodedOp::SetArg(index) => (1, 1, ControlEffect::Next, Some(Operand::Argument(index))),
        DecodedOp::Add
        | DecodedOp::Sub
        | DecodedOp::Mul
        | DecodedOp::Div
        | DecodedOp::Mod
        | DecodedOp::Pow
        | DecodedOp::Shl
        | DecodedOp::Sar
        | DecodedOp::Shr
        | DecodedOp::BitAnd
        | DecodedOp::BitOr
        | DecodedOp::BitXor
        | DecodedOp::Eq
        | DecodedOp::Neq
        | DecodedOp::Lt
        | DecodedOp::Lte
        | DecodedOp::Gt
        | DecodedOp::Gte => (2, 1, ControlEffect::Next, None),
    };
    // The word names the full canonical operation, including its guarded
    // numeric fallback. A Number success path does not narrow these effects.
    let numeric = matches!(
        decoded,
        DecodedOp::Add
            | DecodedOp::Sub
            | DecodedOp::Mul
            | DecodedOp::Div
            | DecodedOp::Mod
            | DecodedOp::Pow
            | DecodedOp::Shl
            | DecodedOp::Sar
            | DecodedOp::Shr
            | DecodedOp::BitAnd
            | DecodedOp::BitOr
            | DecodedOp::BitXor
            | DecodedOp::Eq
            | DecodedOp::Neq
            | DecodedOp::Lt
            | DecodedOp::Lte
            | DecodedOp::Gt
            | DecodedOp::Gte
    );
    // The canonical control-flow contract conservatively permits JavaScript
    // completion for Goto. Preserve that classification even though its
    // present successful run body only updates the next PC.
    let javascript_exception = if numeric
        || matches!(
            decoded,
            DecodedOp::Goto(_) | DecodedOp::IfTrue(_) | DecodedOp::IfFalse(_)
        ) {
        JsExceptionEffect::MayThrow
    } else {
        JsExceptionEffect::None
    };
    instruction.stack_contract()
        == StackEffect {
            popped,
            pushed,
            state: StackStateEffect::Ordinary,
        }
        && instruction.control_effect() == control
        && instruction.operand_contract() == OperandContract([operand, None, None])
        && instruction.potential_effects()
            == PotentialEffects {
                javascript_exception,
                may_call_js: numeric,
                may_allocate: numeric,
            }
}

#[cfg(test)]
mod operation_tests {
    use super::*;

    #[test]
    fn quick_operation_reads_canonical_pcs_without_reconstructing_instruction() {
        let program =
            QuickProgram::build_verified(&[Instruction::PushI32(7), Instruction::Return]).unwrap();
        assert!(program.has_words());
        assert_eq!(program.operation(0), Some(DecodedOp::PushI32(7)));
        assert_eq!(program.operation(1), Some(DecodedOp::GenericCanonical));
        assert_eq!(program.operation(2), None);
        assert_eq!(program.operation(usize::MAX), None);
        let cold = QuickProgram::build_verified(&[Instruction::ReturnUndefined]).unwrap();
        assert!(!cold.has_words());
        assert_eq!(cold.operation(0), None);
    }

    #[test]
    #[should_panic(expected = "published QuickOp word must remain authenticated")]
    fn quick_operation_never_hides_a_corrupt_word_as_canonical_fallback() {
        let corrupt = QuickProgram(ProgramKind::Words(Rc::new(vec![QuickOp(u64::MAX)])));
        let _ = corrupt.operation(0);
    }
}
