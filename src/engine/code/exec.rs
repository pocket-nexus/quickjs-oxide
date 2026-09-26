//! The sole published execution stream. A word has a 16-bit opcode header and
//! a 16-bit short operand. Wide operands occupy following 32-bit words.
//! Compiler instructions are consumed by `encode` and never retained here.

use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use super::bytecode::{
    ApplyKind, ArgumentsKind, DefineMethodKind, DynamicEnvironmentSource, EvalVariableSource,
    Instruction, IteratorCallKind, PrivateNameSource,
};
use super::exec_opcode::Opcode;
use super::function::metadata::{ClosureVariableKind, VariableDefinition};
use super::instruction::{Operand, OperandContract};

const OPCODE_MASK: u16 = 0x03ff;
const COUNT_SHIFT: u16 = 10;
const WIDE_FIRST: u16 = 0x1000;
const WIDTH_SHIFT: u16 = 13;
const RESERVED_MASK: u16 = 0x8000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExecCodeError {
    TooLong,
    BadOpcode,
    BadOperandCount,
    Truncated,
    InvalidHeader,
    InvalidTarget,
    InvalidBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Decoded {
    pub opcode: Opcode,
    pub operands: [u32; 3],
    pub count: u8,
    pub next_pc: u32,
}

impl Decoded {
    #[inline(always)]
    pub fn operand(self, index: usize) -> u32 {
        self.operands[index]
    }

    #[inline(always)]
    pub fn signed_operand(self, index: usize) -> i32 {
        i32::from_ne_bytes(self.operands[index].to_ne_bytes())
    }
}

/// A published word is already authenticated. Keep its extensions in the
/// code stream until the selected handler actually needs them.
#[derive(Clone, Copy)]
pub(crate) struct PublishedDecoded<'a> {
    pub opcode: Opcode,
    word: u32,
    pc: u32,
    words: &'a [Cell<u32>],
    count: u8,
    first_wide: bool,
    pub next_pc: u32,
}

impl PublishedDecoded<'_> {
    #[inline(always)]
    pub fn operand_or_zero(self, index: usize) -> u32 {
        if index >= usize::from(self.count) {
            0
        } else {
            self.operand(index)
        }
    }

    #[inline(always)]
    pub fn operand(self, index: usize) -> u32 {
        if index == 0 && !self.first_wide {
            return u32::from(self.word as u16);
        }
        let offset = if self.first_wide { index + 1 } else { index };
        self.words[self.pc as usize + offset].get()
    }
}

/// Word offsets are the only VM program counters. `boundaries` maps compiler
/// instruction indices to word offsets, with one terminal sentinel. It also
/// resolves fault locations back to the source/debug PC without a hot-loop map.
#[derive(Clone, Debug)]
pub(crate) struct ExecCode {
    words: Rc<[Cell<u32>]>,
    boundaries: Rc<[u32]>,
    /// Compiler assertions inspect the exact prepublication IR. This field is
    /// absent from product builds and never participates in execution.
    #[cfg(test)]
    test_ir: Rc<[Instruction]>,
}

#[derive(Clone, Copy)]
struct EncodedOperand {
    bits: u32,
    short: bool,
    target: bool,
}

impl ExecCode {
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            words: Rc::from([]),
            boundaries: Rc::from([0]),
            #[cfg(test)]
            test_ir: Rc::from([]),
        }
    }

    /// Consume an already authenticated compiler IR. Target widening is fixed
    /// before layout: control-flow targets always use one extension word.
    pub(crate) fn encode_with_locals(
        code: &[Instruction],
        locals: &[VariableDefinition],
    ) -> Result<Self, ExecCodeError> {
        Self::encode_internal(code, Some(locals))
    }

    #[cfg(test)]
    pub(crate) fn encode(code: &[Instruction]) -> Result<Self, ExecCodeError> {
        Self::encode_internal(code, None)
    }

    fn encode_internal(
        code: &[Instruction],
        locals: Option<&[VariableDefinition]>,
    ) -> Result<Self, ExecCodeError> {
        let opcodes = select_opcodes(code, locals);
        let mut boundaries = Vec::with_capacity(code.len() + 1);
        let mut length = 0u32;
        for (source, &opcode) in opcodes.iter().enumerate() {
            boundaries.push(length);
            let operands = published_operands(code, source, opcode);
            let count = operands.len();
            if count != usize::from(opcode.operand_count()) {
                return Err(ExecCodeError::BadOperandCount);
            }
            let width = if count == 0 {
                1
            } else {
                count + usize::from(!operands[0].short)
            };
            length = length
                .checked_add(u32::try_from(width).map_err(|_| ExecCodeError::TooLong)?)
                .ok_or(ExecCodeError::TooLong)?;
        }
        boundaries.push(length);
        let mut words = Vec::with_capacity(length as usize);
        for (source, &opcode) in opcodes.iter().enumerate() {
            let operands = published_operands(code, source, opcode);
            let count = operands.len();
            let first_wide = count > 0 && !operands[0].short;
            let width = 1 + count - usize::from(count != 0 && !first_wide);
            let header = (opcode as u16)
                | ((count as u16) << COUNT_SHIFT)
                | if first_wide { WIDE_FIRST } else { 0 }
                | (((width as u16) - 1) << WIDTH_SHIFT);
            let short = if count > 0 && !first_wide {
                operands[0].bits as u16
            } else {
                0
            };
            words.push((u32::from(header) << 16) | u32::from(short));
            for (index, operand) in operands.iter().enumerate() {
                if index == 0 && !first_wide {
                    continue;
                }
                let bits = if operand.target {
                    let source =
                        usize::try_from(operand.bits).map_err(|_| ExecCodeError::InvalidTarget)?;
                    *boundaries.get(source).ok_or(ExecCodeError::InvalidTarget)?
                } else {
                    operand.bits
                };
                words.push(bits);
            }
        }
        let result = Self {
            words: words.into_iter().map(Cell::new).collect::<Vec<_>>().into(),
            boundaries: boundaries.into(),
            #[cfg(test)]
            test_ir: code.to_vec().into(),
        };
        result.verify()?;
        Ok(result)
    }

    #[inline(always)]
    pub(crate) fn decode(&self, pc: u32) -> Result<Decoded, ExecCodeError> {
        let word = self
            .words
            .get(pc as usize)
            .ok_or(ExecCodeError::InvalidBoundary)?
            .get();
        let header = (word >> 16) as u16;
        if header & RESERVED_MASK != 0 {
            return Err(ExecCodeError::InvalidHeader);
        }
        let opcode = Opcode::from_raw(header & OPCODE_MASK).ok_or(ExecCodeError::BadOpcode)?;
        let count = ((header >> COUNT_SHIFT) & 3) as u8;
        if count != opcode.operand_count() {
            return Err(ExecCodeError::BadOperandCount);
        }
        let first_wide = header & WIDE_FIRST != 0;
        if (count == 0 || first_wide) && word as u16 != 0 {
            return Err(ExecCodeError::InvalidHeader);
        }
        if count == 0 && first_wide {
            return Err(ExecCodeError::InvalidHeader);
        }
        let mut operands = [0; 3];
        let mut cursor = pc.checked_add(1).ok_or(ExecCodeError::TooLong)?;
        for index in 0..usize::from(count) {
            if index == 0 && !first_wide {
                operands[index] = if opcode == Opcode::PushI32 {
                    u32::from_ne_bytes(i32::from(word as i16).to_ne_bytes())
                } else {
                    u32::from(word as u16)
                };
            } else {
                operands[index] = self
                    .words
                    .get(cursor as usize)
                    .ok_or(ExecCodeError::Truncated)?
                    .get();
                cursor = cursor.checked_add(1).ok_or(ExecCodeError::TooLong)?;
            }
        }
        let encoded_width = u32::from(((header >> WIDTH_SHIFT) & 3) + 1);
        if cursor - pc != encoded_width {
            return Err(ExecCodeError::InvalidHeader);
        }
        Ok(Decoded {
            opcode,
            operands,
            count,
            next_pc: cursor,
        })
    }

    /// Published words and all static control-flow targets were verified once.
    /// The execution loop needs only bounds-safe loads and the encoded layout;
    /// it does not recheck the immutable header contract on every visit.
    #[inline(always)]
    pub(crate) fn decode_published(&self, pc: u32) -> Result<PublishedDecoded<'_>, ExecCodeError> {
        let word = self
            .words
            .get(pc as usize)
            .ok_or(ExecCodeError::InvalidBoundary)?
            .get();
        let header = (word >> 16) as u16;
        let opcode = Opcode::from_raw(header & OPCODE_MASK).ok_or(ExecCodeError::BadOpcode)?;
        let count = u32::from((header >> COUNT_SHIFT) & 3);
        let first_wide = header & WIDE_FIRST != 0;
        let next_pc = pc + u32::from(((header >> WIDTH_SHIFT) & 3) + 1);
        Ok(PublishedDecoded {
            opcode,
            word,
            pc,
            words: &self.words,
            count: count as u8,
            first_wide,
            next_pc,
        })
    }

    pub(crate) fn verify(&self) -> Result<(), ExecCodeError> {
        if self.boundaries.first() != Some(&0)
            || self.boundaries.last() != Some(&(self.words.len() as u32))
        {
            return Err(ExecCodeError::InvalidBoundary);
        }
        let mut pc = 0u32;
        let mut entries = HashSet::new();
        for &expected in self.boundaries.iter().take(self.boundaries.len() - 1) {
            if pc != expected {
                return Err(ExecCodeError::InvalidBoundary);
            }
            let decoded = self.decode(pc)?;
            if let Some(index) = decoded.opcode.target_operand() {
                let target = decoded.operand(usize::from(index));
                if target == self.words.len() as u32
                    || self.boundaries.binary_search(&target).is_err()
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
                entries.insert(target);
            }
            if decoded.opcode == Opcode::Gosub {
                entries.insert(decoded.next_pc);
            }
            pc = decoded.next_pc;
        }
        if pc != self.words.len() as u32 {
            return Err(ExecCodeError::InvalidBoundary);
        }
        for source in 0..self.instruction_len() {
            if self.opcode_at_source(source) == Some(Opcode::FieldAccSetDrop) {
                let first = self.decode(self.boundaries[source])?;
                let packed = first.operand(1);
                let end = *self
                    .boundaries
                    .get(source + 6)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                let expected = [
                    Opcode::GetLocal,
                    Opcode::GetFieldCached,
                    Opcode::Add,
                    Opcode::SetLocal,
                    Opcode::Drop,
                ];
                if first.operand(0) > u32::from(u16::MAX)
                    || first.next_pc + 5 != end
                    || (1..6).any(|offset| entries.contains(&self.boundaries[source + offset]))
                    || expected.iter().enumerate().any(|(offset, expected)| {
                        self.opcode_at_source(source + offset + 1) != Some(*expected)
                            || self.boundaries[source + offset + 2]
                                != self.boundaries[source + offset + 1] + 1
                    })
                    || self.decode(self.boundaries[source + 1])?.operand(0) != packed & 0xffff
                    || self.decode(self.boundaries[source + 2])?.operand(0) != packed >> 16
                    || self.decode(self.boundaries[source + 4])?.operand(0) != first.operand(0)
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if self.opcode_at_source(source) == Some(Opcode::DenseAccIndexSetDrop) {
                let first = self.decode(self.boundaries[source])?;
                let packed = first.operand(1);
                let mask = first.operand(2);
                let acc = first.operand(0);
                let end = *self
                    .boundaries
                    .get(source + 9)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                let expected = [
                    Opcode::GetLocal,
                    Opcode::GetLocal,
                    Opcode::PushI32,
                    Opcode::BitAnd,
                    Opcode::GetArrayEl,
                    Opcode::Add,
                    Opcode::SetLocal,
                    Opcode::Drop,
                ];
                if mask & !0xffff != 0
                    || acc > u32::from(u16::MAX)
                    || first.next_pc + 8 != end
                    || (1..9).any(|offset| entries.contains(&self.boundaries[source + offset]))
                    || expected.iter().enumerate().any(|(offset, expected)| {
                        self.opcode_at_source(source + offset + 1) != Some(*expected)
                            || self.boundaries[source + offset + 2]
                                != self.boundaries[source + offset + 1] + 1
                    })
                    || self.decode(self.boundaries[source + 1])?.operand(0) != packed & 0xffff
                    || self.decode(self.boundaries[source + 2])?.operand(0) != packed >> 16
                    || self.decode(self.boundaries[source + 3])?.signed_operand(0)
                        != i32::from(mask as u16 as i16)
                    || self.decode(self.boundaries[source + 7])?.operand(0) != acc
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if matches!(
                self.opcode_at_source(source),
                Some(
                    Opcode::DensePostUpdateLocal
                        | Opcode::DensePostUpdateLocalCheck
                        | Opcode::DensePostUpdateArg
                )
            ) {
                let first = self.decode(self.boundaries[source])?;
                let descriptor = first.operand(1);
                let index_arg = descriptor & 0x1_0000 != 0;
                let index = self.decode(
                    *self
                        .boundaries
                        .get(source + 1)
                        .ok_or(ExecCodeError::InvalidBoundary)?,
                )?;
                let store = self.decode(
                    *self
                        .boundaries
                        .get(source + 3)
                        .ok_or(ExecCodeError::InvalidBoundary)?,
                )?;
                let end = *self
                    .boundaries
                    .get(source + 5)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                if first.operand(0) > u32::from(u16::MAX)
                    || descriptor & !0x3_ffff != 0
                    || !matches!(
                        index.opcode,
                        Opcode::GetArg if index_arg
                    ) && !matches!(
                        index.opcode,
                        Opcode::GetLocal | Opcode::GetLocalCheck if !index_arg
                    )
                    || index.operand(0) != descriptor & 0xffff
                    || self.opcode_at_source(source + 2)
                        != Some(if descriptor & 0x2_0000 != 0 {
                            Opcode::PostDec
                        } else {
                            Opcode::PostInc
                        })
                    || (!index_arg
                        && !matches!(store.opcode, Opcode::PutLocal | Opcode::PutLocalCheck))
                    || (index_arg && store.opcode != Opcode::PutArg)
                    || store.operand(0) != descriptor & 0xffff
                    || self.opcode_at_source(source + 4) != Some(Opcode::GetArrayElDense)
                    || first.operand(2) != end
                    || (1..5).any(|offset| entries.contains(&self.boundaries[source + offset]))
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if matches!(
                self.opcode_at_source(source),
                Some(Opcode::UpdateLocalDiscard | Opcode::UpdateLocalDiscardCheck)
            ) {
                let first = self.decode(self.boundaries[source])?;
                let packed = first.operand(0);
                let index = packed & 0x1fff;
                let descriptor = packed >> 13;
                let operation = self
                    .opcode_at_source(source + 1)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                let expected_operation = match (descriptor & 1 != 0, descriptor & 2 != 0) {
                    (true, false) => Opcode::Inc,
                    (false, false) => Opcode::Dec,
                    (true, true) => Opcode::PostInc,
                    (false, true) => Opcode::PostDec,
                };
                let store = self.decode(
                    *self
                        .boundaries
                        .get(source + 2)
                        .ok_or(ExecCodeError::InvalidBoundary)?,
                )?;
                let put = matches!(store.opcode, Opcode::PutLocal | Opcode::PutLocalCheck);
                let set = matches!(store.opcode, Opcode::SetLocal | Opcode::SetLocalCheck);
                let length = if descriptor & 4 != 0 { 4 } else { 3 };
                let expected_length = if descriptor & 2 != 0 || set { 4 } else { 3 };
                let end = *self
                    .boundaries
                    .get(source + length)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                if descriptor & !7 != 0
                    || packed > u32::from(u16::MAX)
                    || first.next_pc != self.boundaries[source] + 1
                    || operation != expected_operation
                    || (!put && !set)
                    || (descriptor & 2 != 0 && !put)
                    || length != expected_length
                    || store.operand(0) != index
                    || first.next_pc + (length as u32 - 1) != end
                    || (length == 4 && self.opcode_at_source(source + 3) != Some(Opcode::Drop))
                    || (1..length).any(|offset| entries.contains(&self.boundaries[source + offset]))
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if self.opcode_at_source(source) == Some(Opcode::CompareBranchStack) {
                let first = self.decode(self.boundaries[source])?;
                let descriptor = first.operand(0);
                let comparison = Opcode::from_raw((descriptor & 0x3ff) as u16)
                    .ok_or(ExecCodeError::BadOpcode)?;
                let branch = self.decode(
                    *self
                        .boundaries
                        .get(source + 1)
                        .ok_or(ExecCodeError::InvalidBoundary)?,
                )?;
                if descriptor & !0x7ff != 0
                    || !matches!(
                        comparison,
                        Opcode::Lt
                            | Opcode::Lte
                            | Opcode::Gt
                            | Opcode::Gte
                            | Opcode::Eq
                            | Opcode::Neq
                            | Opcode::StrictEq
                            | Opcode::StrictNeq
                    )
                    || entries.contains(&self.boundaries[source + 1])
                    || branch.opcode
                        != if descriptor & 0x400 != 0 {
                            Opcode::IfTrue
                        } else {
                            Opcode::IfFalse
                        }
                    || branch.operand(0) != first.operand(1)
                    || branch.next_pc != first.next_pc + 2
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if matches!(
                self.opcode_at_source(source),
                Some(
                    Opcode::CompareBranchLocal
                        | Opcode::CompareBranchArg
                        | Opcode::CompareBranchLocalLt
                        | Opcode::CompareBranchArgLt
                )
            ) {
                let first = self.decode(self.boundaries[source])?;
                let descriptor = first.operand(1);
                let is_arg = descriptor & 0x1_0000 != 0;
                let comparison = Opcode::from_raw(((descriptor >> 17) & 0x3ff) as u16)
                    .ok_or(ExecCodeError::BadOpcode)?;
                let when_true = descriptor & 0x800_0000 != 0;
                let branch = self.decode(
                    *self
                        .boundaries
                        .get(source + 3)
                        .ok_or(ExecCodeError::InvalidBoundary)?,
                )?;
                if descriptor & 0x7800_0000 != 0
                    || !matches!(
                        comparison,
                        Opcode::Lt
                            | Opcode::Lte
                            | Opcode::Gt
                            | Opcode::Gte
                            | Opcode::Eq
                            | Opcode::Neq
                            | Opcode::StrictEq
                            | Opcode::StrictNeq
                    )
                    || (matches!(
                        first.opcode,
                        Opcode::CompareBranchLocalLt | Opcode::CompareBranchArgLt
                    ) && comparison != Opcode::Lt)
                    || (1..4).any(|offset| entries.contains(&self.boundaries[source + offset]))
                    || self.opcode_at_source(source + 1)
                        != Some(if is_arg {
                            Opcode::GetArg
                        } else {
                            Opcode::GetLocal
                        })
                    || self.decode(self.boundaries[source + 1])?.operand(0) != descriptor & 0xffff
                    || self.opcode_at_source(source + 2) != Some(comparison)
                    || branch.opcode
                        != if when_true {
                            Opcode::IfTrue
                        } else {
                            Opcode::IfFalse
                        }
                    || branch.operand(0) != first.operand(2)
                    || branch.next_pc != first.next_pc + 4
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if matches!(
                self.opcode_at_source(source),
                Some(Opcode::DenseReadLocal | Opcode::DenseReadArg)
            ) {
                let first = self.decode(self.boundaries[source])?;
                let end = *self
                    .boundaries
                    .get(source + 3)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                let key = first.operand(1);
                let is_arg = key & 0x1_0000 != 0;
                if first.operand(2) != end
                    || key & !0x1_ffff != 0
                    || (1..3).any(|offset| entries.contains(&self.boundaries[source + offset]))
                    || self.opcode_at_source(source + 1)
                        != Some(if is_arg {
                            Opcode::GetArg
                        } else {
                            Opcode::GetLocal
                        })
                    || self.decode(self.boundaries[source + 1])?.operand(0) != key & 0xffff
                    || self.opcode_at_source(source + 2) != Some(Opcode::GetArrayElDense)
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if matches!(
                self.opcode_at_source(source),
                Some(
                    Opcode::DenseReadBinaryLocal
                        | Opcode::DenseReadBinaryArg
                        | Opcode::DenseIndexBinaryLocal
                        | Opcode::DenseIndexBinaryArg
                )
            ) {
                let index_binary = matches!(
                    self.opcode_at_source(source),
                    Some(Opcode::DenseIndexBinaryLocal | Opcode::DenseIndexBinaryArg)
                );
                let first = self.decode(self.boundaries[source])?;
                let slots = first.operand(1);
                let descriptor = first.operand(2);
                let operation = Opcode::from_raw((descriptor & 0x3ff) as u16)
                    .ok_or(ExecCodeError::BadOpcode)?;
                let key = self.decode(
                    *self
                        .boundaries
                        .get(source + 1)
                        .ok_or(ExecCodeError::InvalidBoundary)?,
                )?;
                let rhs = self.decode(
                    *self
                        .boundaries
                        .get(source + if index_binary { 2 } else { 3 })
                        .ok_or(ExecCodeError::InvalidBoundary)?,
                )?;
                let end = *self
                    .boundaries
                    .get(source + 5)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                let rhs_mode = (descriptor >> 11) & 3;
                let rhs_expected = match rhs_mode {
                    0 => Opcode::GetLocal,
                    1 => Opcode::GetArg,
                    2 => Opcode::PushI32,
                    _ => return Err(ExecCodeError::InvalidTarget),
                };
                let rhs_value = if rhs_mode == 2 {
                    u32::from_ne_bytes(i32::from((slots >> 16) as i16).to_ne_bytes())
                } else {
                    slots >> 16
                };
                if descriptor & !0x1fff != 0
                    || !(if index_binary {
                        is_dense_index_binary(operation)
                    } else {
                        is_dense_number_binary(operation)
                    })
                    || first.operand(0) > u32::from(u16::MAX)
                    || (1..5).any(|offset| entries.contains(&self.boundaries[source + offset]))
                    || key.opcode
                        != if descriptor & 0x400 != 0 {
                            Opcode::GetArg
                        } else {
                            Opcode::GetLocal
                        }
                    || key.operand(0) != slots & 0xffff
                    || self.opcode_at_source(source + if index_binary { 4 } else { 2 })
                        != Some(Opcode::GetArrayElDense)
                    || rhs.opcode != rhs_expected
                    || rhs.operand(0) != rhs_value
                    || self.opcode_at_source(source + if index_binary { 3 } else { 4 })
                        != Some(operation)
                    || end != first.next_pc + 4
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if matches!(
                self.opcode_at_source(source),
                Some(Opcode::BorrowedFieldLocal | Opcode::BorrowedFieldArg)
            ) {
                let first = self.decode(self.boundaries[source])?;
                let end = *self
                    .boundaries
                    .get(source + 2)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                if first.operand(2) != end
                    || entries.contains(&self.boundaries[source + 1])
                    || self.opcode_at_source(source + 1) != Some(Opcode::GetFieldCached)
                    || self.decode(self.boundaries[source + 1])?.operand(0) != first.operand(1)
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if matches!(
                self.opcode_at_source(source),
                Some(Opcode::DensePreUpdateLocal | Opcode::DensePreUpdateArg)
            ) {
                let first = self.decode(self.boundaries[source])?;
                let end = *self
                    .boundaries
                    .get(source + 5)
                    .ok_or(ExecCodeError::InvalidBoundary)?;
                if first.operand(2) != end
                    || (1..5).any(|offset| entries.contains(&self.boundaries[source + offset]))
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
                let update = first.operand(1);
                let update_is_arg = update & 0x1_0000 != 0;
                let decrement = update & 0x2_0000 != 0;
                if update & !0x3_ffff != 0
                    || self.opcode_at_source(source + 1)
                        != Some(if update_is_arg {
                            Opcode::GetArg
                        } else {
                            Opcode::GetLocal
                        })
                    || self.decode(self.boundaries[source + 1])?.operand(0) != update & 0xffff
                    || self.opcode_at_source(source + 2)
                        != Some(if decrement { Opcode::Dec } else { Opcode::Inc })
                    || self.opcode_at_source(source + 3)
                        != Some(if update_is_arg {
                            Opcode::SetArg
                        } else {
                            Opcode::SetLocal
                        })
                    || self.decode(self.boundaries[source + 3])?.operand(0) != update & 0xffff
                    || self.opcode_at_source(source + 4) != Some(Opcode::GetArrayElDense)
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            if !matches!(
                self.opcode_at_source(source),
                Some(Opcode::NumberLocalInc | Opcode::NumberArgInc)
            ) {
                continue;
            }
            let Some(&middle) = self.boundaries.get(source + 1) else {
                return Err(ExecCodeError::InvalidBoundary);
            };
            let Some(&last) = self.boundaries.get(source + 2) else {
                return Err(ExecCodeError::InvalidBoundary);
            };
            if entries.contains(&middle)
                || entries.contains(&last)
                || self.opcode_at_source(source + 1) != Some(Opcode::PushI32)
                || self.decode(middle)?.signed_operand(0) != 1
                || self.opcode_at_source(source + 2) != Some(Opcode::Add)
            {
                return Err(ExecCodeError::InvalidTarget);
            }
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn source_pc(&self, exec_pc: u32) -> Option<u32> {
        self.boundaries
            .binary_search(&exec_pc)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
    }

    #[inline]
    pub(crate) fn exec_pc(&self, source_pc: u32) -> Option<u32> {
        self.boundaries.get(source_pc as usize).copied()
    }

    pub(crate) fn word_len(&self) -> usize {
        self.words.len()
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn word_storage_identity(&self) -> usize {
        Rc::as_ptr(&self.words) as *const Cell<u32> as usize
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn boundary_storage_identity(&self) -> usize {
        Rc::as_ptr(&self.boundaries) as *const u32 as usize
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn boundary_len(&self) -> usize {
        self.boundaries.len()
    }

    pub(crate) fn instruction_len(&self) -> usize {
        self.boundaries.len().saturating_sub(1)
    }

    #[cfg(test)]
    pub(crate) fn test_ir(&self) -> &[Instruction] {
        &self.test_ir
    }

    pub(crate) fn opcode_at_source(&self, source_pc: usize) -> Option<Opcode> {
        let pc = *self.boundaries.get(source_pc)?;
        self.decode(pc).ok().map(|decoded| decoded.opcode)
    }

    pub(crate) fn opcode_at_exec(&self, exec_pc: usize) -> Option<Opcode> {
        let pc = u32::try_from(exec_pc).ok()?;
        self.boundaries.binary_search(&pc).ok()?;
        self.decode(pc).ok().map(|decoded| decoded.opcode)
    }

    pub(crate) fn opcode_before(&self, resume_pc: usize) -> Option<Opcode> {
        let pc = u32::try_from(resume_pc).ok()?;
        let source = self.boundaries.binary_search(&pc).ok()?;
        source
            .checked_sub(1)
            .and_then(|source| self.opcode_at_source(source))
    }

    pub(crate) fn previous_pc(&self, resume_pc: usize) -> Option<usize> {
        let pc = u32::try_from(resume_pc).ok()?;
        let source = self.boundaries.binary_search(&pc).ok()?;
        Some(self.boundaries[source.saturating_sub(1)] as usize)
    }

    pub(crate) fn is_boundary(&self, exec_pc: usize) -> bool {
        u32::try_from(exec_pc)
            .ok()
            .is_some_and(|pc| self.boundaries.binary_search(&pc).is_ok())
    }

    /// Only the opcode bits change. Operand count, extension width, and all
    /// target/resume offsets remain fixed. Callers must establish any dynamic
    /// guard and the exact paired generic opcode before using this method.
    #[cfg(test)]
    pub(crate) fn quicken_same_width(
        &self,
        pc: u32,
        expected: Opcode,
        replacement: Opcode,
    ) -> Result<bool, ExecCodeError> {
        let legal = matches!(
            (expected, replacement),
            (Opcode::GetLocal, Opcode::NumberLocalInc)
                | (Opcode::NumberLocalInc, Opcode::GetLocal)
                | (Opcode::GetArg, Opcode::NumberArgInc)
                | (Opcode::NumberArgInc, Opcode::GetArg)
                | (Opcode::GetField, Opcode::GetFieldCached)
                | (Opcode::GetFieldCached, Opcode::GetField)
                | (Opcode::GetField2, Opcode::GetField2Cached)
                | (Opcode::GetField2Cached, Opcode::GetField2)
                | (Opcode::GetArrayEl, Opcode::GetArrayElDense)
                | (Opcode::GetArrayElDense, Opcode::GetArrayEl)
                | (Opcode::GetArrayEl2, Opcode::GetArrayEl2Dense)
                | (Opcode::GetArrayEl2Dense, Opcode::GetArrayEl2)
                | (Opcode::GetArrayEl3, Opcode::GetArrayEl3Dense)
                | (Opcode::GetArrayEl3Dense, Opcode::GetArrayEl3)
        );
        if !legal {
            return Err(ExecCodeError::BadOpcode);
        }
        if expected.operand_count() != replacement.operand_count()
            || expected.target_operand() != replacement.target_operand()
        {
            return Err(ExecCodeError::BadOperandCount);
        }
        if self.boundaries.binary_search(&pc).is_err() {
            return Err(ExecCodeError::InvalidBoundary);
        }
        let cell = self
            .words
            .get(pc as usize)
            .ok_or(ExecCodeError::Truncated)?;
        let word = cell.get();
        if ((word >> 16) as u16 & OPCODE_MASK) != expected as u16 {
            return Ok(false);
        }
        let header = ((word >> 16) as u16 & !OPCODE_MASK) | replacement as u16;
        cell.set((u32::from(header) << 16) | (word & 0xffff));
        if let Err(error) = self.verify() {
            cell.set(word);
            return Err(error);
        }
        Ok(true)
    }

    #[cfg(test)]
    pub(crate) fn disassemble(&self) -> Result<String, ExecCodeError> {
        let mut output = String::new();
        for (source, &pc) in self
            .boundaries
            .iter()
            .take(self.instruction_len())
            .enumerate()
        {
            let decoded = self.decode(pc)?;
            use std::fmt::Write;
            write!(&mut output, "{pc:06} [{source:06}] {:?}", decoded.opcode)
                .map_err(|_| ExecCodeError::TooLong)?;
            for operand in decoded.operands.iter().take(decoded.count as usize) {
                write!(&mut output, " {operand}").map_err(|_| ExecCodeError::TooLong)?;
            }
            output.push('\n');
        }
        Ok(output)
    }
}

/// A specialized first word may skip its following generic words only when
/// no control-flow or gosub return can enter the middle of that span.
fn discard_update_shape(
    code: &[Instruction],
    pc: usize,
    locals: Option<&[VariableDefinition]>,
    entries: &HashSet<usize>,
) -> Option<(u32, usize)> {
    let index = match code.get(pc)? {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => *index,
        _ => return None,
    };
    if index >= 1 << 13 {
        return None;
    }
    if locals.is_some_and(|definitions| {
        !definitions
            .get(usize::from(index))
            .is_some_and(|definition| {
                definition.kind == ClosureVariableKind::Normal && !definition.is_const
            })
    }) {
        return None;
    }
    let (increment, postfix) = match code.get(pc + 1)? {
        Instruction::Inc => (true, false),
        Instruction::Dec => (false, false),
        Instruction::PostInc => (true, true),
        Instruction::PostDec => (false, true),
        _ => return None,
    };
    let length = match code.get(pc + 2)? {
        Instruction::PutLocal(target) | Instruction::PutLocalCheck(target) if *target == index => {
            if postfix {
                4
            } else {
                3
            }
        }
        Instruction::SetLocal(target) | Instruction::SetLocalCheck(target)
            if *target == index && !postfix =>
        {
            4
        }
        _ => return None,
    };
    if length == 4 && !matches!(code.get(pc + 3), Some(Instruction::Drop)) {
        return None;
    }
    if (1..length).any(|offset| entries.contains(&(pc + offset))) {
        return None;
    }
    Some((u32::from(increment) | (u32::from(postfix) << 1), length))
}

fn post_update_read_shape(
    code: &[Instruction],
    pc: usize,
    locals: Option<&[VariableDefinition]>,
    entries: &HashSet<usize>,
) -> Option<u32> {
    let base = code.get(pc)?;
    match base {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => {
            if locals.is_some_and(|definitions| {
                !definitions
                    .get(usize::from(*index))
                    .is_some_and(|definition| definition.kind == ClosureVariableKind::Normal)
            }) {
                return None;
            }
        }
        Instruction::GetArg(_) => {}
        _ => return None,
    }
    let (index, arg) = match code.get(pc + 1)? {
        Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => {
            if locals.is_some_and(|definitions| {
                !definitions
                    .get(usize::from(*index))
                    .is_some_and(|definition| {
                        definition.kind == ClosureVariableKind::Normal && !definition.is_const
                    })
            }) {
                return None;
            }
            (*index, false)
        }
        Instruction::GetArg(index) => (*index, true),
        _ => return None,
    };
    let decrement = match code.get(pc + 2)? {
        Instruction::PostInc => false,
        Instruction::PostDec => true,
        _ => return None,
    };
    let store_matches = match code.get(pc + 3)? {
        Instruction::PutLocal(target) | Instruction::PutLocalCheck(target) => {
            !arg && *target == index
        }
        Instruction::PutArg(target) => arg && *target == index,
        _ => false,
    };
    if !store_matches
        || !matches!(code.get(pc + 4), Some(Instruction::GetArrayEl))
        || (1..5).any(|offset| entries.contains(&(pc + offset)))
    {
        return None;
    }
    Some(u32::from(index) | (u32::from(arg) << 16) | (u32::from(decrement) << 17))
}

fn is_dense_number_binary(operation: Opcode) -> bool {
    matches!(
        operation,
        Opcode::Add
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::Div
            | Opcode::BitAnd
            | Opcode::BitOr
            | Opcode::BitXor
            | Opcode::Shl
            | Opcode::Sar
            | Opcode::Shr
    )
}

fn is_dense_index_binary(operation: Opcode) -> bool {
    matches!(operation, Opcode::Add | Opcode::Sub | Opcode::BitAnd)
}

fn dense_acc_index_shape(
    code: &[Instruction],
    pc: usize,
    locals: Option<&[VariableDefinition]>,
    entries: &HashSet<usize>,
) -> Option<(u16, u16, u16, i16)> {
    let [
        Instruction::GetLocal(acc),
        Instruction::GetLocal(base),
        Instruction::GetLocal(key),
        Instruction::PushI32(mask),
        Instruction::BitAnd,
        Instruction::GetArrayEl,
        Instruction::Add,
        Instruction::SetLocal(store),
        Instruction::Drop,
    ] = code.get(pc..pc.checked_add(9)?)?
    else {
        return None;
    };
    if acc != store || (1..9).any(|offset| entries.contains(&(pc + offset))) {
        return None;
    }
    let mask = i16::try_from(*mask).ok()?;
    if let Some(locals) = locals {
        let direct = |index: &u16| {
            locals
                .get(usize::from(*index))
                .is_some_and(|local| local.kind == ClosureVariableKind::Normal)
        };
        if !direct(acc) || !direct(base) || !direct(key) || locals[usize::from(*acc)].is_const {
            return None;
        }
    }
    Some((*acc, *base, *key, mask))
}

fn field_acc_shape(
    code: &[Instruction],
    pc: usize,
    locals: Option<&[VariableDefinition]>,
    entries: &HashSet<usize>,
) -> Option<(u16, u16, u16)> {
    let [
        Instruction::GetLocal(acc),
        Instruction::GetLocal(base),
        Instruction::GetField(field),
        Instruction::Add,
        Instruction::SetLocal(store),
        Instruction::Drop,
    ] = code.get(pc..pc.checked_add(6)?)?
    else {
        return None;
    };
    if acc != store || (1..6).any(|offset| entries.contains(&(pc + offset))) {
        return None;
    }
    let field = u16::try_from(*field).ok()?;
    if let Some(locals) = locals {
        let direct = |index: &u16| {
            locals
                .get(usize::from(*index))
                .is_some_and(|local| local.kind == ClosureVariableKind::Normal)
        };
        if !direct(acc) || !direct(base) || locals[usize::from(*acc)].is_const {
            return None;
        }
    }
    Some((*acc, *base, field))
}

fn select_opcodes(code: &[Instruction], locals: Option<&[VariableDefinition]>) -> Vec<Opcode> {
    let mut entries = HashSet::new();
    for (pc, instruction) in code.iter().enumerate() {
        for operand in instruction.operand_contract().0.into_iter().flatten() {
            if let Operand::Target(target) = operand {
                entries.insert(target as usize);
            }
        }
        if matches!(instruction, Instruction::Gosub(_)) {
            entries.insert(pc + 1);
        }
    }
    let direct_source = |instruction: &Instruction| match instruction {
        Instruction::GetLocal(index) => locals.is_none_or(|definitions| {
            definitions
                .get(usize::from(*index))
                .is_some_and(|definition| definition.kind == ClosureVariableKind::Normal)
        }),
        Instruction::GetArg(_) => true,
        _ => false,
    };
    let writable_source = |instruction: &Instruction| match instruction {
        Instruction::GetLocal(index) => locals.is_none_or(|definitions| {
            definitions
                .get(usize::from(*index))
                .is_some_and(|definition| {
                    definition.kind == ClosureVariableKind::Normal && !definition.is_const
                })
        }),
        Instruction::GetArg(_) => true,
        _ => false,
    };
    let mut opcodes: Vec<_> = code
        .iter()
        .enumerate()
        .map(|(pc, instruction)| {
            if matches!(
                instruction,
                Instruction::GetLocal(_) | Instruction::GetArg(_)
            ) && !direct_source(instruction)
            {
                return Opcode::from_instruction(instruction);
            }
            match instruction {
                Instruction::GetLocal(_)
                    if field_acc_shape(code, pc, locals, &entries).is_some() =>
                {
                    Opcode::FieldAccSetDrop
                }
                Instruction::GetLocal(_)
                    if dense_acc_index_shape(code, pc, locals, &entries).is_some() =>
                {
                    Opcode::DenseAccIndexSetDrop
                }
                Instruction::GetLocal(_) | Instruction::GetLocalCheck(_)
                    if discard_update_shape(code, pc, locals, &entries).is_some() =>
                {
                    if matches!(instruction, Instruction::GetLocalCheck(_)) {
                        Opcode::UpdateLocalDiscardCheck
                    } else {
                        Opcode::UpdateLocalDiscard
                    }
                }
                Instruction::GetLocal(_)
                | Instruction::GetLocalCheck(_)
                | Instruction::GetArg(_)
                    if post_update_read_shape(code, pc, locals, &entries).is_some() =>
                {
                    match instruction {
                        Instruction::GetLocal(_) => Opcode::DensePostUpdateLocal,
                        Instruction::GetLocalCheck(_) => Opcode::DensePostUpdateLocalCheck,
                        Instruction::GetArg(_) => Opcode::DensePostUpdateArg,
                        _ => unreachable!(),
                    }
                }
                Instruction::GetLocal(_) | Instruction::GetArg(_)
                    if matches!(
                        code.get(pc + 1),
                        Some(Instruction::GetLocal(_) | Instruction::GetArg(_))
                    ) && code.get(pc + 1).is_some_and(direct_source)
                        && matches!(
                            code.get(pc + 2),
                            Some(
                                Instruction::GetLocal(_)
                                    | Instruction::GetArg(_)
                                    | Instruction::PushI32(_)
                            )
                        )
                        && code.get(pc + 2).is_some_and(|rhs| match rhs {
                            Instruction::PushI32(value) => i16::try_from(*value).is_ok(),
                            _ => direct_source(rhs),
                        })
                        && code.get(pc + 3).is_some_and(|instruction| {
                            is_dense_index_binary(Opcode::from_instruction(instruction))
                        })
                        && matches!(code.get(pc + 4), Some(Instruction::GetArrayEl))
                        && (1..5).all(|offset| !entries.contains(&(pc + offset))) =>
                {
                    if matches!(instruction, Instruction::GetLocal(_)) {
                        Opcode::DenseIndexBinaryLocal
                    } else {
                        Opcode::DenseIndexBinaryArg
                    }
                }
                Instruction::GetLocal(_) | Instruction::GetArg(_)
                    if matches!(
                        code.get(pc + 1),
                        Some(Instruction::GetLocal(_) | Instruction::GetArg(_))
                    ) && code.get(pc + 1).is_some_and(direct_source)
                        && matches!(code.get(pc + 2), Some(Instruction::GetArrayEl))
                        && matches!(
                            code.get(pc + 3),
                            Some(
                                Instruction::GetLocal(_)
                                    | Instruction::GetArg(_)
                                    | Instruction::PushI32(_)
                            )
                        )
                        && code.get(pc + 3).is_some_and(|rhs| match rhs {
                            Instruction::PushI32(value) => i16::try_from(*value).is_ok(),
                            _ => direct_source(rhs),
                        })
                        && code.get(pc + 4).is_some_and(|instruction| {
                            is_dense_number_binary(Opcode::from_instruction(instruction))
                        })
                        && (1..5).all(|offset| !entries.contains(&(pc + offset))) =>
                {
                    if matches!(instruction, Instruction::GetLocal(_)) {
                        Opcode::DenseReadBinaryLocal
                    } else {
                        Opcode::DenseReadBinaryArg
                    }
                }
                Instruction::GetLocal(_) | Instruction::GetArg(_)
                    if matches!(
                        code.get(pc + 1),
                        Some(Instruction::GetLocal(_) | Instruction::GetArg(_))
                    ) && code.get(pc + 1).is_some_and(writable_source)
                        && matches!(
                            code.get(pc + 2),
                            Some(Instruction::Inc | Instruction::Dec)
                        )
                        && matches!(
                            (code.get(pc + 1), code.get(pc + 3)),
                            (Some(Instruction::GetLocal(a)), Some(Instruction::SetLocal(b)))
                                | (Some(Instruction::GetArg(a)), Some(Instruction::SetArg(b)))
                                if a == b
                        )
                        && matches!(code.get(pc + 4), Some(Instruction::GetArrayEl))
                        && (1..5).all(|offset| !entries.contains(&(pc + offset))) =>
                {
                    if matches!(instruction, Instruction::GetLocal(_)) {
                        Opcode::DensePreUpdateLocal
                    } else {
                        Opcode::DensePreUpdateArg
                    }
                }
                Instruction::GetLocal(_) | Instruction::GetArg(_)
                    if matches!(
                        code.get(pc + 1),
                        Some(Instruction::GetLocal(_) | Instruction::GetArg(_))
                    ) && code.get(pc + 1).is_some_and(direct_source)
                        && matches!(code.get(pc + 2), Some(Instruction::GetArrayEl))
                        && !entries.contains(&(pc + 1))
                        && !entries.contains(&(pc + 2)) =>
                {
                    if matches!(instruction, Instruction::GetLocal(_)) {
                        Opcode::DenseReadLocal
                    } else {
                        Opcode::DenseReadArg
                    }
                }
                Instruction::GetLocal(_) | Instruction::GetArg(_)
                    if matches!(code.get(pc + 1), Some(Instruction::GetField(_)))
                        && !entries.contains(&(pc + 1)) =>
                {
                    if matches!(instruction, Instruction::GetLocal(_)) {
                        Opcode::BorrowedFieldLocal
                    } else {
                        Opcode::BorrowedFieldArg
                    }
                }
                Instruction::GetLocal(_) | Instruction::GetArg(_)
                    if matches!(
                        code.get(pc + 1),
                        Some(Instruction::GetLocal(_) | Instruction::GetArg(_))
                    ) && code.get(pc + 1).is_some_and(direct_source)
                        && matches!(
                            code.get(pc + 2),
                            Some(
                                Instruction::Lt
                                    | Instruction::Lte
                                    | Instruction::Gt
                                    | Instruction::Gte
                                    | Instruction::Eq
                                    | Instruction::Neq
                                    | Instruction::StrictEq
                                    | Instruction::StrictNeq
                            )
                        )
                        && matches!(
                            code.get(pc + 3),
                            Some(Instruction::IfTrue(_) | Instruction::IfFalse(_))
                        )
                        && (1..4).all(|offset| !entries.contains(&(pc + offset))) =>
                {
                    match (instruction, code.get(pc + 2)) {
                        (Instruction::GetLocal(_), Some(Instruction::Lt)) => {
                            Opcode::CompareBranchLocalLt
                        }
                        (Instruction::GetArg(_), Some(Instruction::Lt)) => {
                            Opcode::CompareBranchArgLt
                        }
                        (Instruction::GetLocal(_), _) => Opcode::CompareBranchLocal,
                        _ => Opcode::CompareBranchArg,
                    }
                }
                Instruction::GetLocal(_) | Instruction::GetArg(_)
                    if matches!(code.get(pc + 1), Some(Instruction::PushI32(1)))
                        && matches!(code.get(pc + 2), Some(Instruction::Add))
                        && !entries.contains(&(pc + 1))
                        && !entries.contains(&(pc + 2)) =>
                {
                    if matches!(instruction, Instruction::GetLocal(_)) {
                        Opcode::NumberLocalInc
                    } else {
                        Opcode::NumberArgInc
                    }
                }
                Instruction::GetField(_) => Opcode::GetFieldCached,
                Instruction::GetField2(_) => Opcode::GetField2Cached,
                Instruction::GetArrayEl => Opcode::GetArrayElDense,
                Instruction::GetArrayEl2 => Opcode::GetArrayEl2Dense,
                Instruction::GetArrayEl3 => Opcode::GetArrayEl3Dense,
                _ => Opcode::from_instruction(instruction),
            }
        })
        .collect();
    for pc in 0..code.len().saturating_sub(1) {
        if matches!(
            code[pc],
            Instruction::Lt
                | Instruction::Lte
                | Instruction::Gt
                | Instruction::Gte
                | Instruction::Eq
                | Instruction::Neq
                | Instruction::StrictEq
                | Instruction::StrictNeq
        ) && matches!(
            code[pc + 1],
            Instruction::IfTrue(_) | Instruction::IfFalse(_)
        ) && !entries.contains(&(pc + 1))
            && (pc < 2
                || !matches!(
                    opcodes[pc - 2],
                    Opcode::CompareBranchLocal
                        | Opcode::CompareBranchArg
                        | Opcode::CompareBranchLocalLt
                        | Opcode::CompareBranchArgLt
                ))
        {
            opcodes[pc] = Opcode::CompareBranchStack;
        }
    }
    for pc in 0..code.len() {
        match opcodes[pc] {
            Opcode::FieldAccSetDrop => {
                for offset in 1..6 {
                    opcodes[pc + offset] = Opcode::from_instruction(&code[pc + offset]);
                }
                opcodes[pc + 2] = Opcode::GetFieldCached;
            }
            Opcode::DenseAccIndexSetDrop => {
                for offset in 1..9 {
                    opcodes[pc + offset] = Opcode::from_instruction(&code[pc + offset]);
                }
            }
            Opcode::DenseIndexBinaryLocal | Opcode::DenseIndexBinaryArg => {
                for offset in 1..4 {
                    opcodes[pc + offset] = Opcode::from_instruction(&code[pc + offset]);
                }
                opcodes[pc + 4] = Opcode::GetArrayElDense;
            }
            Opcode::DenseReadBinaryLocal | Opcode::DenseReadBinaryArg => {
                opcodes[pc + 1] = Opcode::from_instruction(&code[pc + 1]);
                opcodes[pc + 3] = Opcode::from_instruction(&code[pc + 3]);
            }
            _ => {}
        }
    }
    opcodes
}

fn encoded_operands(contract: OperandContract) -> Vec<EncodedOperand> {
    contract
        .0
        .into_iter()
        .flatten()
        .map(encode_operand)
        .collect()
}

fn published_operands(code: &[Instruction], source: usize, opcode: Opcode) -> Vec<EncodedOperand> {
    let mut operands = encoded_operands(code[source].operand_contract());
    if opcode == Opcode::FieldAccSetDrop {
        let (base, field) = match (&code[source + 1], &code[source + 2]) {
            (Instruction::GetLocal(base), Instruction::GetField(field)) => (*base, *field),
            _ => unreachable!("field accumulator selected without direct operands"),
        };
        operands.push(EncodedOperand {
            bits: u32::from(base) | (field << 16),
            short: false,
            target: false,
        });
    } else if opcode == Opcode::DenseAccIndexSetDrop {
        let (base, key, mask) = match (&code[source + 1], &code[source + 2], &code[source + 3]) {
            (
                Instruction::GetLocal(base),
                Instruction::GetLocal(key),
                Instruction::PushI32(mask),
            ) => (*base, *key, *mask),
            _ => unreachable!("dense accumulator selected without direct operands"),
        };
        operands.push(EncodedOperand {
            bits: u32::from(base) | (u32::from(key) << 16),
            short: false,
            target: false,
        });
        operands.push(EncodedOperand {
            bits: u32::from(mask as i16 as u16),
            short: false,
            target: false,
        });
    } else if matches!(
        opcode,
        Opcode::DensePostUpdateLocal
            | Opcode::DensePostUpdateLocalCheck
            | Opcode::DensePostUpdateArg
    ) {
        let (index, arg) = match code[source + 1] {
            Instruction::GetLocal(index) | Instruction::GetLocalCheck(index) => (index, false),
            Instruction::GetArg(index) => (index, true),
            _ => unreachable!("post-update span has no direct index"),
        };
        operands.push(EncodedOperand {
            bits: u32::from(index)
                | (u32::from(arg) << 16)
                | if matches!(code[source + 2], Instruction::PostDec) {
                    0x2_0000
                } else {
                    0
                },
            short: false,
            target: false,
        });
        operands.push(EncodedOperand {
            bits: u32::try_from(source + 5).expect("execution source exceeds u32"),
            short: false,
            target: true,
        });
    } else if matches!(
        opcode,
        Opcode::UpdateLocalDiscard | Opcode::UpdateLocalDiscardCheck
    ) {
        let operation = &code[source + 1];
        let increment = matches!(operation, Instruction::Inc | Instruction::PostInc);
        let postfix = matches!(operation, Instruction::PostInc | Instruction::PostDec);
        let length = if postfix
            || matches!(
                code[source + 2],
                Instruction::SetLocal(_) | Instruction::SetLocalCheck(_)
            ) {
            4
        } else {
            3
        };
        let descriptor =
            u32::from(increment) | (u32::from(postfix) << 1) | (u32::from(length == 4) << 2);
        operands[0].bits |= descriptor << 13;
    } else if matches!(
        opcode,
        Opcode::DensePreUpdateLocal | Opcode::DensePreUpdateArg
    ) {
        let (index, is_arg) = match code[source + 1] {
            Instruction::GetLocal(index) => (index, false),
            Instruction::GetArg(index) => (index, true),
            _ => unreachable!("specialized opcode selected without its index source"),
        };
        operands.push(EncodedOperand {
            bits: u32::from(index)
                | if is_arg { 0x1_0000 } else { 0 }
                | if matches!(code[source + 2], Instruction::Dec) {
                    0x2_0000
                } else {
                    0
                },
            short: false,
            target: false,
        });
        operands.push(EncodedOperand {
            bits: u32::try_from(source + 5).expect("execution source exceeds u32"),
            short: false,
            target: true,
        });
    } else if matches!(
        opcode,
        Opcode::DenseReadBinaryLocal
            | Opcode::DenseReadBinaryArg
            | Opcode::DenseIndexBinaryLocal
            | Opcode::DenseIndexBinaryArg
    ) {
        let index_binary = matches!(
            opcode,
            Opcode::DenseIndexBinaryLocal | Opcode::DenseIndexBinaryArg
        );
        let (key_index, key_arg) = match code[source + 1] {
            Instruction::GetLocal(index) => (index, false),
            Instruction::GetArg(index) => (index, true),
            _ => unreachable!("binary dense read has no direct key"),
        };
        let (rhs_bits, rhs_mode) = match code[source + if index_binary { 2 } else { 3 }] {
            Instruction::GetLocal(index) => (index, 0u32),
            Instruction::GetArg(index) => (index, 1u32),
            Instruction::PushI32(value) => (value as i16 as u16, 2u32),
            _ => unreachable!("binary dense read has no direct rhs"),
        };
        operands.push(EncodedOperand {
            bits: u32::from(key_index) | (u32::from(rhs_bits) << 16),
            short: false,
            target: false,
        });
        operands.push(EncodedOperand {
            bits: u32::from(Opcode::from_instruction(
                &code[source + if index_binary { 3 } else { 4 }],
            ) as u16)
                | (u32::from(key_arg) << 10)
                | (rhs_mode << 11),
            short: false,
            target: false,
        });
    } else if matches!(opcode, Opcode::DenseReadLocal | Opcode::DenseReadArg) {
        let (index, is_arg) = match code[source + 1] {
            Instruction::GetLocal(index) => (index, false),
            Instruction::GetArg(index) => (index, true),
            _ => unreachable!("dense read selected without numeric index source"),
        };
        operands.push(EncodedOperand {
            bits: u32::from(index) | if is_arg { 0x1_0000 } else { 0 },
            short: false,
            target: false,
        });
        operands.push(EncodedOperand {
            bits: u32::try_from(source + 3).expect("execution source exceeds u32"),
            short: false,
            target: true,
        });
    } else if opcode == Opcode::CompareBranchStack {
        let compare = Opcode::from_instruction(&code[source]) as u16;
        let (target, when_true) = match code[source + 1] {
            Instruction::IfTrue(target) => (target, true),
            Instruction::IfFalse(target) => (target, false),
            _ => unreachable!("comparison selected without branch"),
        };
        operands = vec![
            EncodedOperand {
                bits: u32::from(compare) | if when_true { 0x400 } else { 0 },
                short: true,
                target: false,
            },
            EncodedOperand {
                bits: target,
                short: false,
                target: true,
            },
        ];
    } else if matches!(
        opcode,
        Opcode::BorrowedFieldLocal | Opcode::BorrowedFieldArg
    ) {
        let Instruction::GetField(index) = code[source + 1] else {
            unreachable!("borrowed field selected without static field read")
        };
        operands.push(EncodedOperand {
            bits: index,
            short: false,
            target: false,
        });
        operands.push(EncodedOperand {
            bits: u32::try_from(source + 2).expect("execution source exceeds u32"),
            short: false,
            target: true,
        });
    } else if matches!(
        opcode,
        Opcode::CompareBranchLocal
            | Opcode::CompareBranchArg
            | Opcode::CompareBranchLocalLt
            | Opcode::CompareBranchArgLt
    ) {
        let (index, is_arg) = match code[source + 1] {
            Instruction::GetLocal(index) => (index, false),
            Instruction::GetArg(index) => (index, true),
            _ => unreachable!("comparison selected without second direct source"),
        };
        let compare = Opcode::from_instruction(&code[source + 2]) as u16;
        let (target, when_true) = match code[source + 3] {
            Instruction::IfTrue(target) => (target, true),
            Instruction::IfFalse(target) => (target, false),
            _ => unreachable!("comparison selected without branch"),
        };
        operands.push(EncodedOperand {
            bits: u32::from(index)
                | if is_arg { 0x1_0000 } else { 0 }
                | (u32::from(compare) << 17)
                | if when_true { 0x800_0000 } else { 0 },
            short: false,
            target: false,
        });
        operands.push(EncodedOperand {
            bits: target,
            short: false,
            target: true,
        });
    }
    operands
}

fn encode_operand(operand: Operand) -> EncodedOperand {
    let (bits, short, target) = match operand {
        Operand::Integer(value) => (value as u32, i16::try_from(value).is_ok(), false),
        Operand::Target(value) => (value, false, true),
        Operand::AtomValueIndex(value) | Operand::Constant { index: value, .. } => {
            (value, u16::try_from(value).is_ok(), false)
        }
        Operand::Local(value)
        | Operand::Argument(value)
        | Operand::Closure(value)
        | Operand::PrivateLocal(value)
        | Operand::ArgumentCount(value)
        | Operand::RestStart(value)
        | Operand::ElementCount(value)
        | Operand::EvalEnvironment(value) => (u32::from(value), true, false),
        Operand::EvalSource(source) => (encode_eval_source(source), false, false),
        Operand::DynamicSource(source) => (encode_dynamic_source(source), false, false),
        Operand::PrivateSource(source) => (encode_private_source(source), false, false),
        Operand::ArgumentsKind(ArgumentsKind::Mapped) => (0, true, false),
        Operand::ArgumentsKind(ArgumentsKind::Unmapped) => (1, true, false),
        Operand::ApplyKind(ApplyKind::Call) => (0, true, false),
        Operand::ApplyKind(ApplyKind::Construct) => (1, true, false),
        Operand::IteratorCallKind(IteratorCallKind::ReturnWithValue) => (0, true, false),
        Operand::IteratorCallKind(IteratorCallKind::ThrowWithValue) => (1, true, false),
        Operand::IteratorCallKind(IteratorCallKind::ReturnWithoutValue) => (2, true, false),
        Operand::MethodKind(DefineMethodKind::Method) => (0, true, false),
        Operand::MethodKind(DefineMethodKind::Getter) => (1, true, false),
        Operand::MethodKind(DefineMethodKind::Setter) => (2, true, false),
        Operand::Enumerable(value) | Operand::HasHeritage(value) => (u32::from(value), true, false),
        Operand::IteratorDepth(value) | Operand::StackDepth { depth: value, .. } => {
            (u32::from(value), true, false)
        }
    };
    EncodedOperand {
        bits,
        short,
        target,
    }
}

fn encode_eval_source(source: EvalVariableSource) -> u32 {
    match source {
        EvalVariableSource::Local(index) => u32::from(index),
        EvalVariableSource::Closure(index) => 0x1_0000 | u32::from(index),
    }
}

fn encode_dynamic_source(source: DynamicEnvironmentSource) -> u32 {
    match source {
        DynamicEnvironmentSource::Eval(source) => encode_eval_source(source),
        DynamicEnvironmentSource::With(source) => match source {
            super::bytecode::WithObjectSource::Local(index) => 0x2_0000 | u32::from(index),
            super::bytecode::WithObjectSource::Closure(index) => 0x3_0000 | u32::from(index),
        },
    }
}

fn encode_private_source(source: PrivateNameSource) -> u32 {
    match source {
        PrivateNameSource::Local(index) => u32::from(index),
        PrivateNameSource::Closure(index) => 0x1_0000 | u32::from(index),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wide_operands_and_branch_targets_use_word_boundaries() {
        let code = ExecCode::encode(&[
            Instruction::PushConst(0x1_0000),
            Instruction::IfFalse(3),
            Instruction::PushI32(-1),
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(code.word_len(), 6);
        assert_eq!(code.exec_pc(0), Some(0));
        assert_eq!(code.exec_pc(1), Some(2));
        assert_eq!(code.exec_pc(2), Some(4));
        assert_eq!(code.exec_pc(3), Some(5));
        assert_eq!(code.decode(2).unwrap().operand(0), 5);
        assert_eq!(code.decode(4).unwrap().signed_operand(0), -1);
        assert_eq!(code.source_pc(3), None);
        code.verify().unwrap();
        assert!(code.disassemble().unwrap().contains("PushConst"));
        let header = code.words[0].get();
        code.words[0].set(header ^ (1 << (16 + WIDTH_SHIFT)));
        assert_eq!(code.verify(), Err(ExecCodeError::InvalidHeader));
    }

    #[test]
    fn invalid_targets_and_extension_entry_are_rejected() {
        assert_eq!(
            ExecCode::encode(&[Instruction::Goto(1)]).unwrap_err(),
            ExecCodeError::InvalidTarget
        );
        let code =
            ExecCode::encode(&[Instruction::PushConst(0x1_0000), Instruction::Goto(0)]).unwrap();
        assert_eq!(code.opcode_at_exec(1), None);
        assert_eq!(code.opcode_before(2), Some(Opcode::PushConst));
        code.words[3].set(1);
        assert_eq!(code.verify(), Err(ExecCodeError::InvalidTarget));
    }

    #[test]
    fn catch_targets_and_wide_signed_operands_share_verified_boundaries() {
        let code = ExecCode::encode(&[
            Instruction::Catch(3),
            Instruction::PushI32(i32::MIN),
            Instruction::Return,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        let handler = code.exec_pc(3).unwrap();
        assert_eq!(code.decode(0).unwrap().operand(0), handler);
        assert_eq!(code.decode(2).unwrap().signed_operand(0), i32::MIN);
        assert_eq!(code.source_pc(handler), Some(3));
        code.words[1].set(3);
        assert_eq!(code.verify(), Err(ExecCodeError::InvalidTarget));
    }

    #[test]
    fn numeric_span_is_selected_only_without_internal_entry() {
        let span = ExecCode::encode(&[
            Instruction::GetLocal(0),
            Instruction::PushI32(1),
            Instruction::Add,
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(span.opcode_at_source(0), Some(Opcode::NumberLocalInc));
        assert_eq!(span.opcode_at_source(1), Some(Opcode::PushI32));
        assert_eq!(span.opcode_at_source(2), Some(Opcode::Add));
        assert_eq!(span.exec_pc(3), Some(3));
        let entered = ExecCode::encode(&[
            Instruction::Goto(2),
            Instruction::GetLocal(0),
            Instruction::PushI32(1),
            Instruction::Add,
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetLocal));
        assert_eq!(
            entered.quicken_same_width(2, Opcode::GetLocal, Opcode::NumberLocalInc),
            Err(ExecCodeError::InvalidTarget)
        );
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetLocal));
    }

    #[test]
    fn dense_preupdate_commits_only_from_a_single_verified_entry() {
        let code = ExecCode::encode(&[
            Instruction::GetLocal(0),
            Instruction::GetLocal(1),
            Instruction::Inc,
            Instruction::SetLocal(1),
            Instruction::GetArrayEl,
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::DensePreUpdateLocal));
        assert_eq!(code.decode(0).unwrap().operand(1), 1);
        assert_eq!(code.decode(0).unwrap().operand(2), code.exec_pc(5).unwrap());
        code.verify().unwrap();

        let entered = ExecCode::encode(&[
            Instruction::Goto(3),
            Instruction::GetLocal(0),
            Instruction::GetLocal(1),
            Instruction::Inc,
            Instruction::SetLocal(1),
            Instruction::GetArrayEl,
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetLocal));
    }

    #[test]
    fn dense_accumulator_index_span_has_one_verified_entry() {
        let body = [
            Instruction::GetLocal(0),
            Instruction::GetLocal(1),
            Instruction::GetLocal(2),
            Instruction::PushI32(3),
            Instruction::BitAnd,
            Instruction::GetArrayEl,
            Instruction::Add,
            Instruction::SetLocal(0),
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ];
        let code = ExecCode::encode(&body).unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::DenseAccIndexSetDrop));
        assert_eq!(code.opcode_at_source(5), Some(Opcode::GetArrayEl));
        assert_eq!(code.decode(0).unwrap().operand(1), 1 | (2 << 16));
        assert_eq!(code.exec_pc(9), Some(11));
        code.verify().unwrap();

        let mut entered = vec![Instruction::Goto(5)];
        entered.extend_from_slice(&body);
        let entered = ExecCode::encode(&entered).unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetLocal));

        code.words[2].set(0x1_0003);
        assert_eq!(code.verify(), Err(ExecCodeError::InvalidTarget));
    }

    #[test]
    fn field_accumulator_span_uses_the_generic_field_on_miss() {
        let body = [
            Instruction::GetLocal(0),
            Instruction::GetLocal(1),
            Instruction::GetField(2),
            Instruction::Add,
            Instruction::SetLocal(0),
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ];
        let code = ExecCode::encode(&body).unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::FieldAccSetDrop));
        assert_eq!(code.opcode_at_source(2), Some(Opcode::GetFieldCached));
        assert_eq!(code.decode(0).unwrap().operand(1), 1 | (2 << 16));
        assert_eq!(code.exec_pc(6), Some(7));
        code.verify().unwrap();

        let mut entered = vec![Instruction::Goto(3)];
        entered.extend_from_slice(&body);
        let entered = ExecCode::encode(&entered).unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetLocal));

        code.words[1].set(1 | (3 << 16));
        assert_eq!(code.verify(), Err(ExecCodeError::InvalidTarget));
    }

    #[test]
    fn published_dense_and_field_reads_skip_only_authenticated_words() {
        let code = ExecCode::encode(&[
            Instruction::GetArg(0),
            Instruction::GetLocal(1),
            Instruction::GetArrayEl,
            Instruction::GetLocal(2),
            Instruction::GetField(0x1_0000),
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::DenseReadArg));
        assert_eq!(code.opcode_at_source(3), Some(Opcode::BorrowedFieldLocal));
        assert_eq!(code.decode(0).unwrap().operand(2), code.exec_pc(3).unwrap());
        assert_eq!(
            code.decode(code.exec_pc(3).unwrap()).unwrap().operand(2),
            code.exec_pc(5).unwrap()
        );
        code.verify().unwrap();

        let entered = ExecCode::encode(&[
            Instruction::Goto(2),
            Instruction::GetArg(0),
            Instruction::GetLocal(1),
            Instruction::GetArrayEl,
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetArg));
    }

    #[test]
    fn dense_read_binary_requires_an_unentered_fixed_width_tail() {
        use Instruction::*;
        let source = [GetArg(0), GetArg(1), GetArrayEl, GetArg(2), Sub, Return];
        let code = ExecCode::encode(&source).unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::DenseReadBinaryArg));
        assert_eq!(
            code.decode(0).unwrap().next_pc + 4,
            code.exec_pc(5).unwrap()
        );
        code.verify().unwrap();

        let immediate = ExecCode::encode(&[
            GetLocal(0),
            GetArg(0),
            GetArrayEl,
            PushI32(0x3fff),
            BitAnd,
            Return,
        ])
        .unwrap();
        assert_eq!(
            immediate.opcode_at_source(0),
            Some(Opcode::DenseReadBinaryLocal)
        );
        immediate.verify().unwrap();

        let entered = ExecCode::encode(&[
            Goto(4),
            GetArg(0),
            GetArg(1),
            GetArrayEl,
            GetArg(2),
            Sub,
            Return,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::DenseReadArg));
    }

    #[test]
    fn dense_index_binary_verifies_the_middle_and_tail() {
        use Instruction::*;
        let code =
            ExecCode::encode(&[GetArg(0), GetArg(1), PushI32(1), Add, GetArrayEl, Return]).unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::DenseIndexBinaryArg));
        assert_eq!(
            code.decode(0).unwrap().next_pc + 4,
            code.exec_pc(5).unwrap()
        );
        code.verify().unwrap();

        let entered = ExecCode::encode(&[
            Goto(3),
            GetArg(0),
            GetArg(1),
            PushI32(1),
            Add,
            GetArrayEl,
            Return,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetArg));
    }

    #[test]
    fn comparison_branch_carries_the_verified_word_target() {
        let code = ExecCode::encode(&[
            Instruction::GetLocal(0),
            Instruction::GetArg(1),
            Instruction::Lt,
            Instruction::IfFalse(5),
            Instruction::Goto(4),
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::CompareBranchLocalLt));
        assert_eq!(code.decode(0).unwrap().operand(2), code.exec_pc(5).unwrap());
        code.verify().unwrap();

        let entered = ExecCode::encode(&[
            Instruction::Goto(2),
            Instruction::GetLocal(0),
            Instruction::GetArg(1),
            Instruction::Lt,
            Instruction::IfFalse(5),
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetLocal));
    }

    #[test]
    fn specialized_local_comparison_rejects_corrupt_descriptor() {
        let code = ExecCode::encode(&[
            Instruction::GetArg(0),
            Instruction::GetLocal(1),
            Instruction::Lt,
            Instruction::IfFalse(4),
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::CompareBranchArgLt));
        let descriptor = code.words[1].get();
        code.words[1].set(descriptor | 0x0800_0000);
        assert_eq!(code.verify(), Err(ExecCodeError::InvalidTarget));
        code.words[1].set((descriptor & !(0x3ff << 17)) | (u32::from(Opcode::Gt as u16) << 17));
        assert_eq!(code.verify(), Err(ExecCodeError::InvalidTarget));
    }

    #[test]
    fn stack_comparison_branch_authenticates_its_single_entry_and_fallthrough() {
        let code = ExecCode::encode(&[
            Instruction::PushI32(1),
            Instruction::PushI32(2),
            Instruction::Lt,
            Instruction::IfFalse(5),
            Instruction::Return,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(code.opcode_at_source(2), Some(Opcode::CompareBranchStack));
        let start = code.exec_pc(2).unwrap();
        let span = code.decode(start).unwrap();
        assert_eq!(span.operand(1), code.exec_pc(5).unwrap());
        assert_eq!(span.next_pc + 2, code.exec_pc(4).unwrap());
        code.verify().unwrap();

        let entered = ExecCode::encode(&[
            Instruction::Goto(4),
            Instruction::PushI32(1),
            Instruction::PushI32(2),
            Instruction::Lt,
            Instruction::IfFalse(6),
            Instruction::Return,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(3), Some(Opcode::Lt));
    }

    #[test]
    fn resident_decoder_matches_verified_layouts() {
        let code = ExecCode::encode(&[
            Instruction::PushI32(12),
            Instruction::PushI32(i32::MIN),
            Instruction::GetField(0x1_0000),
            Instruction::Lt,
            Instruction::IfFalse(6),
            Instruction::Return,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        for source in 0..code.instruction_len() {
            let pc = code.exec_pc(source as u32).unwrap();
            let published = code.decode_published(pc).unwrap();
            let verified = code.decode(pc).unwrap();
            assert_eq!(published.opcode, verified.opcode);
            assert_eq!(published.next_pc, verified.next_pc);
            for index in 0..usize::from(verified.count) {
                let expected = if index == 0
                    && verified.opcode == Opcode::PushI32
                    && verified.next_pc == pc + 1
                {
                    u32::from(verified.operand(index) as u16)
                } else {
                    verified.operand(index)
                };
                assert_eq!(published.operand(index), expected);
            }
            for index in usize::from(verified.count)..3 {
                assert_eq!(published.operand_or_zero(index), 0);
            }
        }
    }

    #[test]
    fn publication_omits_candidates_for_ineligible_local_metadata() {
        let local = |kind, is_const| VariableDefinition {
            name: None,
            is_lexical: false,
            is_const,
            is_parameter_initializer: false,
            kind,
        };
        let read = [
            Instruction::GetLocal(0),
            Instruction::PushI32(1),
            Instruction::Add,
            Instruction::Return,
        ];
        let normal = [local(ClosureVariableKind::Normal, false)];
        let special = [local(ClosureVariableKind::FunctionName, false)];
        assert_eq!(
            ExecCode::encode_with_locals(&read, &normal)
                .unwrap()
                .opcode_at_source(0),
            Some(Opcode::NumberLocalInc)
        );
        assert_eq!(
            ExecCode::encode_with_locals(&read, &special)
                .unwrap()
                .opcode_at_source(0),
            Some(Opcode::GetLocal)
        );

        let update = [
            Instruction::GetLocal(0),
            Instruction::GetLocal(1),
            Instruction::Inc,
            Instruction::SetLocal(1),
            Instruction::GetArrayEl,
            Instruction::Return,
        ];
        let writable = [normal[0], local(ClosureVariableKind::Normal, false)];
        let constant = [normal[0], local(ClosureVariableKind::Normal, true)];
        assert_eq!(
            ExecCode::encode_with_locals(&update, &writable)
                .unwrap()
                .opcode_at_source(0),
            Some(Opcode::DensePreUpdateLocal)
        );
        assert_eq!(
            ExecCode::encode_with_locals(&update, &constant)
                .unwrap()
                .opcode_at_source(0),
            Some(Opcode::GetLocal)
        );
    }

    #[test]
    fn discarded_update_skips_only_an_unentered_authenticated_span() {
        let code = ExecCode::encode(&[
            Instruction::GetLocal(0),
            Instruction::PostInc,
            Instruction::PutLocal(0),
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::UpdateLocalDiscard));
        assert_eq!(code.decode(0).unwrap().operand(0), 7 << 13);
        assert_eq!(
            code.decode(0).unwrap().next_pc + 3,
            code.exec_pc(4).unwrap()
        );
        code.verify().unwrap();

        let wide_index = ExecCode::encode(&[
            Instruction::GetLocal(8192),
            Instruction::Inc,
            Instruction::SetLocal(8192),
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(wide_index.opcode_at_source(0), Some(Opcode::GetLocal));

        let checked = ExecCode::encode(&[
            Instruction::GetLocalCheck(0),
            Instruction::Inc,
            Instruction::SetLocalCheck(0),
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(
            checked.opcode_at_source(0),
            Some(Opcode::UpdateLocalDiscardCheck)
        );

        let entered = ExecCode::encode(&[
            Instruction::Goto(4),
            Instruction::GetLocal(0),
            Instruction::PostInc,
            Instruction::PutLocal(0),
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetLocal));
    }

    #[test]
    fn post_update_array_read_keeps_generic_words_at_verified_positions() {
        let code = ExecCode::encode(&[
            Instruction::GetArg(0),
            Instruction::GetLocalCheck(1),
            Instruction::PostInc,
            Instruction::PutLocalCheck(1),
            Instruction::GetArrayEl,
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(code.opcode_at_source(0), Some(Opcode::DensePostUpdateArg));
        assert_eq!(code.decode(0).unwrap().operand(1), 1);
        assert_eq!(code.decode(0).unwrap().operand(2), code.exec_pc(5).unwrap());
        code.verify().unwrap();

        let entered = ExecCode::encode(&[
            Instruction::Goto(3),
            Instruction::GetArg(0),
            Instruction::GetLocal(1),
            Instruction::PostDec,
            Instruction::PutLocal(1),
            Instruction::GetArrayEl,
            Instruction::Return,
        ])
        .unwrap();
        assert_eq!(entered.opcode_at_source(1), Some(Opcode::GetArg));
    }
}
