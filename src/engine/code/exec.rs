//! The sole published execution stream. A word has a 16-bit opcode header and
//! a 16-bit short operand. Wide operands occupy following 32-bit words.
//! Compiler instructions are consumed by `encode` and never retained here.

use crate::engine::heap::{BytecodeConstant, RawValue};
use std::cell::Cell;
use std::collections::HashSet;
use std::rc::Rc;

use super::bytecode::{
    ApplyKind, ArgumentsKind, DefineMethodKind, DynamicEnvironmentSource, EvalVariableSource,
    Instruction, IteratorCallKind, PrivateNameSource,
};
use super::exec_opcode::Opcode;
use super::function::metadata::{ClosureVariableKind, VariableDefinition};
use super::initialization::definite_initialization_entries;
use super::instruction::{Operand, OperandContract, StackStateEffect};
#[cfg(feature = "profiling")]
use super::region::RejectedNumericSite;
use super::region::{
    ArrayProductSource, ArrayReadSource, DirectSource, NumberSource, NumericOperation,
    NumericRegion, PublishedNumericRegion, UpdateDelta,
};

const OPCODE_MASK: u16 = 0x03ff;
const COUNT_SHIFT: u16 = 10;
const WIDE_FIRST: u16 = 0x1000;
const WIDTH_SHIFT: u16 = 13;
const RESERVED_MASK: u16 = 0x8000;

#[cfg(test)]
thread_local! {
    static SUPPRESS_NUMERIC_REGIONS: Cell<bool> = const { Cell::new(false) };
}

#[cfg(test)]
pub(crate) fn without_numeric_regions<T>(run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SUPPRESS_NUMERIC_REGIONS.set(self.0);
        }
    }
    let previous = SUPPRESS_NUMERIC_REGIONS.replace(true);
    let _restore = Restore(previous);
    run()
}

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
    /// The original first operation of a guarded region has its own physical
    /// entry, but keeps the guard's logical source/debug identity.
    guarded_fallbacks: Option<Rc<[GuardedFallback]>>,
    regions: Option<Rc<[PublishedNumericRegion]>>,
    product_sources: Option<Rc<[ArrayProductSource]>>,
    copy_sources: Option<Rc<[ArrayReadSource]>>,
    #[cfg(feature = "profiling")]
    rejected_numeric_sites: Rc<[RejectedNumericSite]>,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GuardedFallback {
    source: u32,
    pc: u32,
}

#[derive(Clone, Copy)]
struct ScheduledEntry {
    source: usize,
    opcode: Opcode,
    guard: bool,
}

impl ExecCode {
    #[cfg(test)]
    pub(crate) fn empty() -> Self {
        Self {
            words: Rc::from([]),
            boundaries: Rc::from([0]),
            guarded_fallbacks: None,
            regions: None,
            product_sources: None,
            copy_sources: None,
            #[cfg(feature = "profiling")]
            rejected_numeric_sites: Rc::from([]),
            #[cfg(test)]
            test_ir: Rc::from([]),
        }
    }

    /// Consume an already authenticated compiler IR. Target widening is fixed
    /// before layout: control-flow targets always use one extension word.
    pub(crate) fn encode_with_locals(
        code: &[Instruction],
        locals: &[VariableDefinition],
        arguments: &[VariableDefinition],
        regions: &[NumericRegion],
        constants: &[BytecodeConstant],
    ) -> Result<Self, ExecCodeError> {
        #[cfg(test)]
        let regions = if SUPPRESS_NUMERIC_REGIONS.get() {
            &[]
        } else {
            regions
        };
        Self::encode_internal(code, Some(locals), Some(arguments), regions, constants)
    }

    #[cfg(test)]
    pub(crate) fn encode(code: &[Instruction]) -> Result<Self, ExecCodeError> {
        Self::encode_internal(code, None, None, &[], &[])
    }

    fn encode_internal(
        code: &[Instruction],
        locals: Option<&[VariableDefinition]>,
        arguments: Option<&[VariableDefinition]>,
        regions: &[NumericRegion],
        constants: &[BytecodeConstant],
    ) -> Result<Self, ExecCodeError> {
        validate_region_plans(code, locals, arguments, regions, constants)?;
        // A guarded entry must leave an ordinary first operation at F. Most
        // functions need one selection pass; reselect only if a product
        // candidate cannot publish its selected first operation.
        let mut opcodes = select_opcodes(code, locals, regions);
        let selected_regions: Vec<_> = regions
            .iter()
            .filter(|region| {
                !matches!(
                    region.operation,
                    NumericOperation::UpdateElement {
                        delta: UpdateDelta::ArrayProduct(_)
                    }
                ) || opcodes[region.start as usize] == direct_opcode(region.array)
            })
            .copied()
            .collect();
        if selected_regions.len() != regions.len() {
            opcodes = select_opcodes(code, locals, &selected_regions);
        }
        let regions = selected_regions.as_slice();
        let mut region_ids = vec![None; code.len()];
        for (id, region) in regions.iter().enumerate() {
            region_ids[region.start as usize] = Some(id);
        }
        let mut schedule = Vec::with_capacity(code.len() + regions.len());
        for (source, &opcode) in opcodes.iter().enumerate() {
            if let Some(id) = region_ids[source] {
                if matches!(
                    regions[id].operation,
                    NumericOperation::UpdateElement {
                        delta: UpdateDelta::ArrayProduct(_)
                    }
                ) {
                    schedule.push(ScheduledEntry {
                        source,
                        opcode: region_opcode(regions[id].operation),
                        guard: true,
                    });
                }
            }
            schedule.push(ScheduledEntry {
                source,
                opcode,
                guard: false,
            });
        }
        let mut boundaries = Vec::with_capacity(code.len() + 1);
        let mut guarded_fallbacks = Vec::new();
        let mut length = 0u32;
        for entry in &schedule {
            let source = entry.source;
            let opcode = entry.opcode;
            if boundaries.len() == source {
                boundaries.push(length);
            }
            if !entry.guard
                && region_ids[source].is_some_and(|id| {
                    matches!(
                        regions[id].operation,
                        NumericOperation::UpdateElement {
                            delta: UpdateDelta::ArrayProduct(_)
                        }
                    )
                })
            {
                guarded_fallbacks.push(GuardedFallback {
                    source: source as u32,
                    pc: length,
                });
            }
            let operands = published_operands(code, source, opcode, regions, &region_ids);
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
        for entry in &schedule {
            let source = entry.source;
            let opcode = entry.opcode;
            let mut operands = published_operands(code, source, opcode, regions, &region_ids);
            if entry.guard {
                operands[2] = EncodedOperand {
                    bits: guarded_fallbacks
                        .iter()
                        .find(|fallback| fallback.source == source as u32)
                        .ok_or(ExecCodeError::InvalidTarget)?
                        .pc,
                    short: false,
                    target: false,
                };
            }
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
        let mut product_sources = Vec::new();
        let mut copy_sources = Vec::new();
        let published_regions = (!regions.is_empty())
            .then(|| {
                regions
                    .iter()
                    .map(|region| {
                        let mut published = publish_numeric_region(region, &boundaries)?;
                        published.producer_index = match region.operation {
                            NumericOperation::UpdateElement {
                                delta: UpdateDelta::ArrayProduct(source),
                            } => {
                                let index = u32::try_from(product_sources.len())
                                    .map_err(|_| ExecCodeError::InvalidTarget)?;
                                product_sources.push(source);
                                Some(index)
                            }
                            NumericOperation::CopyElement { source } => {
                                let index = u32::try_from(copy_sources.len())
                                    .map_err(|_| ExecCodeError::InvalidTarget)?;
                                copy_sources.push(source);
                                Some(index)
                            }
                            _ => None,
                        };
                        Ok(published)
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map(Rc::from)
            })
            .transpose()?;
        #[cfg(feature = "profiling")]
        let (rejected_numeric_sites, omitted_numeric_sites) =
            collect_rejected_numeric_sites(code, locals, arguments, regions, &opcodes, &boundaries);
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_compiled_numeric_rejections(
            &rejected_numeric_sites,
            omitted_numeric_sites,
        );
        let result = Self {
            words: words.into_iter().map(Cell::new).collect::<Vec<_>>().into(),
            boundaries: boundaries.into(),
            guarded_fallbacks: (!guarded_fallbacks.is_empty()).then(|| Rc::from(guarded_fallbacks)),
            regions: published_regions,
            product_sources: (!product_sources.is_empty()).then(|| product_sources.into()),
            copy_sources: (!copy_sources.is_empty()).then(|| copy_sources.into()),
            #[cfg(feature = "profiling")]
            rejected_numeric_sites: rejected_numeric_sites.into(),
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
        for (index, slot) in operands.iter_mut().enumerate().take(usize::from(count)) {
            if index == 0 && !first_wide {
                *slot = if opcode == Opcode::PushI32 {
                    u32::from_ne_bytes(i32::from(word as i16).to_ne_bytes())
                } else {
                    u32::from(word as u16)
                };
            } else {
                *slot = self
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
        let mut seen_regions =
            vec![false; self.regions.as_ref().map_or(0, |regions| regions.len())];
        for source in 0..self.instruction_len() {
            let expected = self.boundaries[source];
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
            if is_region_opcode(decoded.opcode) {
                let Some(seen) = seen_regions.get_mut(decoded.operand(0) as usize) else {
                    return Err(ExecCodeError::InvalidTarget);
                };
                if *seen {
                    return Err(ExecCodeError::InvalidTarget);
                }
                *seen = true;
                for index in [1, 2] {
                    let target = decoded.operand(index);
                    let guarded = index == 2
                        && self.guarded_fallbacks().iter().any(|fallback| {
                            fallback.source == source as u32
                                && fallback.pc == target
                                && target == decoded.next_pc
                        });
                    if target == self.words.len() as u32
                        || !guarded && self.boundaries.binary_search(&target).is_err()
                    {
                        return Err(ExecCodeError::InvalidTarget);
                    }
                    if index == 1 {
                        entries.insert(target);
                    }
                }
            }
            if decoded.opcode == Opcode::Gosub {
                entries.insert(decoded.next_pc);
            }
            pc = decoded.next_pc;
            if let Some(fallback) = self
                .guarded_fallbacks()
                .iter()
                .find(|fallback| fallback.source == source as u32)
            {
                if pc != fallback.pc {
                    return Err(ExecCodeError::InvalidTarget);
                }
                pc = self.decode(pc)?.next_pc;
            }
        }
        if pc != self.words.len() as u32 {
            return Err(ExecCodeError::InvalidBoundary);
        }
        if seen_regions.iter().any(|seen| !seen) {
            return Err(ExecCodeError::InvalidTarget);
        }
        for source in 0..self.instruction_len() {
            if self.opcode_at_source(source).is_some_and(is_region_opcode) {
                self.verify_numeric_region(source, &entries)?;
            }
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

    pub(crate) fn numeric_region(&self, index: u32) -> Option<&PublishedNumericRegion> {
        self.regions.as_deref()?.get(index as usize)
    }

    pub(crate) fn product_source(&self, index: u32) -> Option<ArrayProductSource> {
        self.product_sources
            .as_deref()?
            .get(index as usize)
            .copied()
    }

    pub(crate) fn copy_source(&self, index: u32) -> Option<ArrayReadSource> {
        self.copy_sources.as_deref()?.get(index as usize).copied()
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn rejected_numeric_sites(&self) -> &[RejectedNumericSite] {
        &self.rejected_numeric_sites
    }

    fn verify_numeric_region(
        &self,
        source: usize,
        entries: &HashSet<u32>,
    ) -> Result<(), ExecCodeError> {
        let first = self.decode(self.boundaries[source])?;
        let published = self
            .numeric_region(first.operand(0))
            .ok_or(ExecCodeError::InvalidTarget)?;
        let product = if first.opcode == Opcode::NumericArrayUpdateElement {
            published
                .producer_index
                .map(|index| {
                    self.product_source(index)
                        .ok_or(ExecCodeError::InvalidTarget)
                })
                .transpose()?
        } else {
            None
        };
        let copy = if first.opcode == Opcode::NumericArrayCopyElement {
            Some(
                self.copy_source(
                    published
                        .producer_index
                        .ok_or(ExecCodeError::InvalidTarget)?,
                )
                .ok_or(ExecCodeError::InvalidTarget)?,
            )
        } else {
            None
        };
        if published.producer_index.is_some() && product.is_none() && copy.is_none()
            || product.is_some() && !matches!(published.value, NumberSource::Immediate(0))
        {
            return Err(ExecCodeError::InvalidTarget);
        }
        if published.shared_update_index
            != product.is_some_and(|product| {
                matches!(
                    (published.index, product.index),
                    (NumberSource::Direct(left), NumberSource::Direct(right)) if left == right
                )
            })
        {
            return Err(ExecCodeError::InvalidTarget);
        }
        let start = source;
        let end = start
            + match first.opcode {
                Opcode::NumericArrayAccumulate => 9,
                Opcode::NumericArrayStoreProduct => 7,
                Opcode::NumericArrayCopyElement => 8,
                Opcode::NumericArrayAddPreInc => 6,
                Opcode::NumericArrayStoreAndLocal => 4,
                Opcode::NumericArrayUpdateElement => {
                    if product.is_some() {
                        12
                    } else {
                        8
                    }
                }
                Opcode::NumericArrayCompareBranch => 6,
                _ => return Err(ExecCodeError::InvalidTarget),
            };
        let operation = match first.opcode {
            Opcode::NumericArrayAccumulate => NumericOperation::Accumulate {
                destination: published.destination,
                scale: published.value,
                checked: published.checked,
            },
            Opcode::NumericArrayStoreProduct => NumericOperation::StoreProduct {
                destination: published.destination,
                scale: published.value,
                checked: published.checked,
            },
            Opcode::NumericArrayCopyElement => NumericOperation::CopyElement {
                source: copy.ok_or(ExecCodeError::InvalidTarget)?,
            },
            Opcode::NumericArrayAddPreInc => NumericOperation::AddPreInc,
            Opcode::NumericArrayStoreAndLocal => NumericOperation::StoreElementAndLocal {
                destination: published.destination,
                checked: published.checked,
            },
            Opcode::NumericArrayUpdateElement => NumericOperation::UpdateElement {
                delta: match product {
                    Some(product) => UpdateDelta::ArrayProduct(product),
                    None => UpdateDelta::Number(published.value),
                },
            },
            Opcode::NumericArrayCompareBranch => NumericOperation::CompareBranch {
                rhs: published.value,
                comparison: published.comparison,
                when_true: published.when_true,
                target: self
                    .source_pc(first.operand(1))
                    .ok_or(ExecCodeError::InvalidTarget)?,
            },
            _ => unreachable!(),
        };
        let region = NumericRegion {
            start: source as u32,
            end: end as u32,
            array: published.array,
            index: published.index,
            operation,
            peak: published.peak,
        };
        let success = match region.operation {
            NumericOperation::CompareBranch { target, .. } => self.exec_pc(target),
            _ => self.boundaries.get(end).copied(),
        };
        if start != source
            || end >= self.instruction_len()
            || published.fallthrough_pc != self.boundaries[end]
            || first.opcode != region_opcode(region.operation)
            || Some(first.operand(1)) != success
            || first.operand(2)
                != if product.is_some() {
                    self.guarded_fallbacks()
                        .iter()
                        .find(|fallback| fallback.source == start as u32)
                        .ok_or(ExecCodeError::InvalidTarget)?
                        .pc
                } else {
                    self.boundaries[start + 1]
                }
            || (start + 1..end).any(|index| entries.contains(&self.boundaries[index]))
        {
            return Err(ExecCodeError::InvalidTarget);
        }
        let mut expected = [Opcode::Nop; 12];
        let expected_len = match region.operation {
            NumericOperation::Accumulate {
                destination: _,
                scale,
                checked,
            } => {
                expected[..9].copy_from_slice(&[
                    if checked {
                        Opcode::GetLocalCheck
                    } else {
                        Opcode::GetLocal
                    },
                    direct_opcode(region.array),
                    number_opcode(region.index),
                    Opcode::GetArrayEl,
                    number_opcode(scale),
                    Opcode::Mul,
                    Opcode::Add,
                    if checked {
                        Opcode::SetLocalCheck
                    } else {
                        Opcode::SetLocal
                    },
                    Opcode::Drop,
                ]);
                9
            }
            NumericOperation::StoreProduct {
                destination: _,
                scale,
                checked,
            } => {
                expected[..7].copy_from_slice(&[
                    direct_opcode(region.array),
                    number_opcode(region.index),
                    Opcode::GetArrayEl,
                    number_opcode(scale),
                    Opcode::Mul,
                    if checked {
                        Opcode::SetLocalCheck
                    } else {
                        Opcode::SetLocal
                    },
                    Opcode::Drop,
                ]);
                7
            }
            NumericOperation::CopyElement { source } => {
                expected[..8].copy_from_slice(&[
                    direct_opcode(region.array),
                    number_opcode(region.index),
                    direct_opcode(source.array),
                    number_opcode(source.index),
                    Opcode::GetArrayEl,
                    Opcode::Insert3,
                    Opcode::PutArrayEl,
                    Opcode::Drop,
                ]);
                8
            }
            NumericOperation::AddPreInc => {
                let NumberSource::Direct(index) = region.index else {
                    return Err(ExecCodeError::InvalidTarget);
                };
                expected[..6].copy_from_slice(&[
                    direct_opcode(region.array),
                    direct_opcode(index),
                    Opcode::Inc,
                    if matches!(index, DirectSource::CheckedLocal(_)) {
                        Opcode::SetLocalCheck
                    } else {
                        Opcode::SetLocal
                    },
                    Opcode::GetArrayEl,
                    Opcode::Add,
                ]);
                6
            }
            NumericOperation::StoreElementAndLocal { checked, .. } => {
                expected[..4].copy_from_slice(&[
                    Opcode::Insert3,
                    Opcode::PutArrayEl,
                    if checked {
                        Opcode::SetLocalCheck
                    } else {
                        Opcode::SetLocal
                    },
                    Opcode::Drop,
                ]);
                4
            }
            NumericOperation::UpdateElement { delta } => match delta {
                UpdateDelta::Number(source) => {
                    expected[..8].copy_from_slice(&[
                        direct_opcode(region.array),
                        number_opcode(region.index),
                        Opcode::GetArrayEl3,
                        number_opcode(source),
                        Opcode::Add,
                        Opcode::Insert3,
                        Opcode::PutArrayEl,
                        Opcode::Drop,
                    ]);
                    8
                }
                UpdateDelta::ArrayProduct(product) => {
                    expected.copy_from_slice(&[
                        direct_opcode(region.array),
                        number_opcode(region.index),
                        Opcode::GetArrayEl3,
                        number_opcode(product.scale),
                        direct_opcode(product.array),
                        number_opcode(product.index),
                        Opcode::GetArrayEl,
                        Opcode::Mul,
                        Opcode::Add,
                        Opcode::Insert3,
                        Opcode::PutArrayEl,
                        Opcode::Drop,
                    ]);
                    12
                }
            },
            NumericOperation::CompareBranch {
                rhs,
                comparison,
                when_true,
                ..
            } => {
                expected[..6].copy_from_slice(&[
                    direct_opcode(region.array),
                    number_opcode(region.index),
                    Opcode::GetArrayEl,
                    number_opcode(rhs),
                    comparison,
                    if when_true {
                        Opcode::IfTrue
                    } else {
                        Opcode::IfFalse
                    },
                ]);
                6
            }
        };
        if end != start + expected_len {
            return Err(ExecCodeError::InvalidTarget);
        }
        if product.is_some() {
            let fallback = self
                .guarded_fallbacks()
                .iter()
                .find(|fallback| fallback.source == start as u32)
                .ok_or(ExecCodeError::InvalidTarget)?;
            let decoded = self.decode(fallback.pc)?;
            if decoded.opcode != expected[0]
                || decoded.operand(0) != direct_index(region.array)
                || decoded.next_pc != self.boundaries[start + 1]
            {
                return Err(ExecCodeError::InvalidTarget);
            }
        }
        for (offset, &opcode) in expected[..expected_len].iter().enumerate().skip(1) {
            let decoded = self.decode(self.boundaries[start + offset])?;
            let compatible = if product.is_some() {
                matches!(
                    (opcode, decoded.opcode),
                    (Opcode::GetArrayEl3, Opcode::GetArrayEl3Dense)
                        | (Opcode::GetArrayEl, Opcode::GetArrayElDense)
                        | (Opcode::GetLocal, Opcode::DenseReadLocal)
                        | (Opcode::GetArg, Opcode::DenseReadArg)
                )
            } else {
                false
            };
            if decoded.opcode != opcode && !compatible {
                return Err(ExecCodeError::InvalidTarget);
            }
        }
        let (array_offset, index_offset) = match region.operation {
            NumericOperation::Accumulate { .. } => (1, 2),
            _ => (0, 1),
        };
        if !matches!(
            region.operation,
            NumericOperation::StoreElementAndLocal { .. }
        ) && (array_offset != 0
            && !verify_direct_operand(self, start + array_offset, region.array)?
            || !verify_number_operand(self, start + index_offset, region.index)?)
        {
            return Err(ExecCodeError::InvalidTarget);
        }
        match region.operation {
            NumericOperation::Accumulate {
                destination, scale, ..
            } => {
                if self.decode(self.boundaries[start + 7])?.operand(0) != u32::from(destination)
                    || !verify_number_operand(self, start + 4, scale)?
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            NumericOperation::StoreProduct {
                destination, scale, ..
            } => {
                if self.decode(self.boundaries[start + 5])?.operand(0) != u32::from(destination)
                    || !verify_number_operand(self, start + 3, scale)?
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            NumericOperation::CopyElement { source } => {
                if !verify_direct_operand(self, start + 2, source.array)?
                    || !verify_number_operand(self, start + 3, source.index)?
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            NumericOperation::AddPreInc => {
                let NumberSource::Direct(index) = region.index else {
                    return Err(ExecCodeError::InvalidTarget);
                };
                let slot = match index {
                    DirectSource::Local(slot) | DirectSource::CheckedLocal(slot) => slot,
                    DirectSource::Argument(_) => return Err(ExecCodeError::InvalidTarget),
                };
                if self.decode(self.boundaries[start + 3])?.operand(0) != u32::from(slot)
                    || published.peak != 2
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            NumericOperation::StoreElementAndLocal {
                destination,
                checked: _,
            } => {
                if self.decode(self.boundaries[start + 2])?.operand(0) != u32::from(destination)
                    || published.peak != 1
                    || !matches!(published.array, DirectSource::Local(0))
                    || !matches!(published.index, NumberSource::Immediate(0))
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
            NumericOperation::UpdateElement { delta } => match delta {
                UpdateDelta::Number(source) => {
                    if !verify_number_operand(self, start + 3, source)? {
                        return Err(ExecCodeError::InvalidTarget);
                    }
                }
                UpdateDelta::ArrayProduct(product) => {
                    if !verify_number_operand(self, start + 3, product.scale)?
                        || !verify_direct_operand(self, start + 4, product.array)?
                        || !verify_number_operand(self, start + 5, product.index)?
                    {
                        return Err(ExecCodeError::InvalidTarget);
                    }
                }
            },
            NumericOperation::CompareBranch { rhs, target, .. } => {
                if !verify_number_operand(self, start + 3, rhs)?
                    || self.decode(self.boundaries[start + 5])?.operand(0)
                        != self.boundaries[target as usize]
                {
                    return Err(ExecCodeError::InvalidTarget);
                }
            }
        }
        Ok(())
    }

    #[inline]
    fn guarded_fallbacks(&self) -> &[GuardedFallback] {
        self.guarded_fallbacks.as_deref().unwrap_or(&[])
    }

    #[inline]
    pub(crate) fn source_pc(&self, exec_pc: u32) -> Option<u32> {
        self.boundaries
            .binary_search(&exec_pc)
            .ok()
            .and_then(|index| u32::try_from(index).ok())
            .or_else(|| {
                self.guarded_fallbacks()
                    .iter()
                    .find(|fallback| fallback.pc == exec_pc)
                    .map(|fallback| fallback.source)
            })
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

    #[cfg(feature = "profiling")]
    pub(crate) fn region_storage_identity(&self) -> usize {
        self.regions.as_ref().map_or(0, |regions| {
            Rc::as_ptr(regions) as *const PublishedNumericRegion as usize
        })
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn region_len(&self) -> usize {
        self.regions.as_ref().map_or(0, |regions| regions.len())
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn product_source_storage(&self) -> (usize, usize) {
        self.product_sources.as_ref().map_or((0, 0), |sources| {
            (
                Rc::as_ptr(sources) as *const ArrayProductSource as usize,
                sources.len(),
            )
        })
    }

    #[cfg(feature = "profiling")]
    pub(crate) fn copy_source_storage(&self) -> (usize, usize) {
        self.copy_sources.as_ref().map_or((0, 0), |sources| {
            (
                Rc::as_ptr(sources) as *const ArrayReadSource as usize,
                sources.len(),
            )
        })
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
        self.is_boundary(exec_pc).then_some(())?;
        self.decode(pc).ok().map(|decoded| decoded.opcode)
    }

    pub(crate) fn opcode_before(&self, resume_pc: usize) -> Option<Opcode> {
        let previous = self.previous_pc(resume_pc)?;
        self.opcode_at_exec(previous)
    }

    pub(crate) fn previous_pc(&self, resume_pc: usize) -> Option<usize> {
        let pc = u32::try_from(resume_pc).ok()?;
        let source = self.source_pc(pc)? as usize;
        if self
            .guarded_fallbacks()
            .iter()
            .any(|fallback| fallback.pc == pc)
        {
            return Some(self.boundaries[source] as usize);
        }
        if source == 0 {
            return Some(0);
        }
        Some(
            self.guarded_fallbacks()
                .iter()
                .find(|fallback| fallback.source == (source - 1) as u32)
                .map_or(self.boundaries[source - 1], |fallback| fallback.pc) as usize,
        )
    }

    pub(crate) fn is_boundary(&self, exec_pc: usize) -> bool {
        u32::try_from(exec_pc).ok().is_some_and(|pc| {
            self.boundaries.binary_search(&pc).is_ok()
                || self
                    .guarded_fallbacks()
                    .iter()
                    .any(|fallback| fallback.pc == pc)
        })
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
        if !self.is_boundary(pc as usize) {
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

fn direct_opcode(source: DirectSource) -> Opcode {
    match source {
        DirectSource::Local(_) => Opcode::GetLocal,
        DirectSource::CheckedLocal(_) => Opcode::GetLocalCheck,
        DirectSource::Argument(_) => Opcode::GetArg,
    }
}

fn direct_index(source: DirectSource) -> u32 {
    match source {
        DirectSource::Local(index)
        | DirectSource::CheckedLocal(index)
        | DirectSource::Argument(index) => u32::from(index),
    }
}

fn number_opcode(source: NumberSource) -> Opcode {
    match source {
        NumberSource::Direct(source) => direct_opcode(source),
        NumberSource::Immediate(_) => Opcode::PushI32,
        NumberSource::Constant { .. } => Opcode::PushConst,
    }
}

fn number_matches_raw(source: NumberSource, raw: &BytecodeConstant) -> bool {
    let NumberSource::Constant { value, .. } = source else {
        return false;
    };
    let actual = match raw {
        BytecodeConstant::Value(RawValue::Int(value)) => f64::from(*value),
        BytecodeConstant::Value(RawValue::Float(value)) => *value,
        _ => return false,
    };
    value.float().to_bits() == actual.to_bits()
}

fn instruction_matches_number(
    instruction: &Instruction,
    source: NumberSource,
    constants: &[BytecodeConstant],
) -> bool {
    match (instruction, source) {
        (Instruction::GetLocal(a), NumberSource::Direct(DirectSource::Local(b)))
        | (Instruction::GetLocalCheck(a), NumberSource::Direct(DirectSource::CheckedLocal(b)))
        | (Instruction::GetArg(a), NumberSource::Direct(DirectSource::Argument(b))) => *a == b,
        (Instruction::PushI32(a), NumberSource::Immediate(b)) => *a == b,
        (Instruction::PushConst(a), NumberSource::Constant { index, .. }) => {
            *a == index
                && constants
                    .get(index as usize)
                    .is_some_and(|raw| number_matches_raw(source, raw))
        }
        _ => false,
    }
}

fn instruction_matches_direct(instruction: &Instruction, source: DirectSource) -> bool {
    matches!(
        (instruction, source),
        (Instruction::GetLocal(a), DirectSource::Local(b))
            | (Instruction::GetLocalCheck(a), DirectSource::CheckedLocal(b))
            | (Instruction::GetArg(a), DirectSource::Argument(b)) if *a == b
    )
}

fn region_opcode(operation: NumericOperation) -> Opcode {
    match operation {
        NumericOperation::Accumulate { .. } => Opcode::NumericArrayAccumulate,
        NumericOperation::StoreProduct { .. } => Opcode::NumericArrayStoreProduct,
        NumericOperation::CopyElement { .. } => Opcode::NumericArrayCopyElement,
        NumericOperation::AddPreInc => Opcode::NumericArrayAddPreInc,
        NumericOperation::StoreElementAndLocal { .. } => Opcode::NumericArrayStoreAndLocal,
        NumericOperation::UpdateElement { .. } => Opcode::NumericArrayUpdateElement,
        NumericOperation::CompareBranch { .. } => Opcode::NumericArrayCompareBranch,
    }
}

fn publish_numeric_region(
    region: &NumericRegion,
    boundaries: &[u32],
) -> Result<PublishedNumericRegion, ExecCodeError> {
    let fallthrough_pc = *boundaries
        .get(region.end as usize)
        .ok_or(ExecCodeError::InvalidTarget)?;
    let (value, update_product, _copy_source, destination, checked, comparison, when_true) =
        match region.operation {
            NumericOperation::Accumulate {
                destination,
                scale,
                checked,
            }
            | NumericOperation::StoreProduct {
                destination,
                scale,
                checked,
            } => (scale, None, None, destination, checked, Opcode::Nop, false),
            NumericOperation::CopyElement { source } => (
                NumberSource::Immediate(0),
                None,
                Some(source),
                0,
                false,
                Opcode::Nop,
                false,
            ),
            NumericOperation::AddPreInc => (
                NumberSource::Immediate(0),
                None,
                None,
                0,
                false,
                Opcode::Nop,
                false,
            ),
            NumericOperation::StoreElementAndLocal {
                destination,
                checked,
            } => (
                NumberSource::Immediate(0),
                None,
                None,
                destination,
                checked,
                Opcode::Nop,
                false,
            ),
            NumericOperation::UpdateElement { delta } => match delta {
                UpdateDelta::Number(source) => (source, None, None, 0, false, Opcode::Nop, false),
                UpdateDelta::ArrayProduct(product) => (
                    NumberSource::Immediate(0),
                    Some(product),
                    None,
                    0,
                    false,
                    Opcode::Nop,
                    false,
                ),
            },
            NumericOperation::CompareBranch {
                rhs,
                comparison,
                when_true,
                ..
            } => (rhs, None, None, 0, false, comparison, when_true),
        };
    let shared_update_index = update_product.is_some_and(|product| {
        matches!(
            (region.index, product.index),
            (NumberSource::Direct(left), NumberSource::Direct(right)) if left == right
        )
    });
    Ok(PublishedNumericRegion {
        array: region.array,
        index: region.index,
        value,
        producer_index: None,
        shared_update_index,
        destination,
        checked,
        comparison,
        when_true,
        fallthrough_pc,
        peak: region.peak,
    })
}

#[cfg(feature = "profiling")]
fn collect_rejected_numeric_sites(
    code: &[Instruction],
    locals: Option<&[VariableDefinition]>,
    arguments: Option<&[VariableDefinition]>,
    regions: &[NumericRegion],
    opcodes: &[Opcode],
    boundaries: &[u32],
) -> (Vec<RejectedNumericSite>, usize) {
    const MAX_PER_FUNCTION: usize = 4_096;
    let Some(locals) = locals else {
        return (Vec::new(), 0);
    };
    let Some(arguments) = arguments else {
        return (Vec::new(), 0);
    };
    let mut entries = vec![false; code.len()];
    if let Some(entry) = entries.first_mut() {
        *entry = true;
    }
    for (pc, instruction) in code.iter().enumerate() {
        let control = instruction.control_effect();
        if let Some(target) = control.target()
            && let Some(entry) = entries.get_mut(target as usize)
        {
            *entry = true;
        }
        if control.ends_block()
            && let Some(entry) = entries.get_mut(pc + 1)
        {
            *entry = true;
        }
    }
    let initialization = if locals.iter().any(|local| local.is_lexical) {
        definite_initialization_entries(code, locals.len())
    } else {
        vec![None; code.len()]
    };
    let direct = |instruction: Option<&Instruction>| match instruction {
        Some(Instruction::GetLocal(index) | Instruction::GetLocalCheck(index)) => locals
            .get(*index as usize)
            .is_some_and(|local| local.kind == ClosureVariableKind::Normal),
        Some(Instruction::GetArg(index)) => (*index as usize) < arguments.len(),
        _ => false,
    };
    let number = |instruction: Option<&Instruction>| {
        direct(instruction)
            || matches!(
                instruction,
                Some(Instruction::PushI32(_) | Instruction::PushConst(_))
            )
    };
    let mut sites = Vec::new();
    let mut omitted = 0;
    let store = |pc: usize| {
        matches!(
            code.get(pc),
            Some(Instruction::SetLocal(_) | Instruction::SetLocalCheck(_))
        )
    };
    let comparison = |pc: usize| {
        matches!(
            code.get(pc),
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
    };
    for (array_pc, instruction) in code.iter().enumerate() {
        // Recognize a bounded lowered operation skeleton, including a single
        // computed-delta shape. Nearby unrelated arithmetic is not a site.
        let (family, start, end, discarded, delta_shape) = match instruction {
            Instruction::GetArrayEl3 if array_pc >= 2 => {
                if matches!(code.get(array_pc + 4), Some(Instruction::GetArrayEl))
                    && matches!(code.get(array_pc + 5), Some(Instruction::Mul))
                    && matches!(code.get(array_pc + 6), Some(Instruction::Add))
                    && matches!(code.get(array_pc + 7), Some(Instruction::Insert3))
                    && matches!(code.get(array_pc + 8), Some(Instruction::PutArrayEl))
                {
                    (
                        "update_element",
                        array_pc - 2,
                        array_pc + 10,
                        matches!(code.get(array_pc + 9), Some(Instruction::Drop)),
                        1,
                    )
                } else if matches!(code.get(array_pc + 3), Some(Instruction::Mul))
                    && matches!(code.get(array_pc + 4), Some(Instruction::Add))
                    && matches!(code.get(array_pc + 5), Some(Instruction::Insert3))
                    && matches!(code.get(array_pc + 6), Some(Instruction::PutArrayEl))
                {
                    (
                        "update_element",
                        array_pc - 2,
                        array_pc + 8,
                        matches!(code.get(array_pc + 7), Some(Instruction::Drop)),
                        2,
                    )
                } else if matches!(code.get(array_pc + 2), Some(Instruction::Add))
                    && matches!(code.get(array_pc + 3), Some(Instruction::Insert3))
                    && matches!(code.get(array_pc + 4), Some(Instruction::PutArrayEl))
                {
                    (
                        "update_element",
                        array_pc - 2,
                        array_pc + 6,
                        matches!(code.get(array_pc + 5), Some(Instruction::Drop)),
                        0,
                    )
                } else {
                    continue;
                }
            }
            Instruction::GetArrayEl
                if array_pc >= 3
                    && matches!(code.get(array_pc + 2), Some(Instruction::Mul))
                    && matches!(code.get(array_pc + 3), Some(Instruction::Add))
                    && store(array_pc + 4) =>
            {
                (
                    "accumulate",
                    array_pc - 3,
                    array_pc + 6,
                    matches!(code.get(array_pc + 5), Some(Instruction::Drop)),
                    0,
                )
            }
            Instruction::GetArrayEl
                if array_pc >= 2
                    && matches!(code.get(array_pc + 2), Some(Instruction::Mul))
                    && store(array_pc + 3) =>
            {
                (
                    "store_product",
                    array_pc - 2,
                    array_pc + 5,
                    matches!(code.get(array_pc + 4), Some(Instruction::Drop)),
                    0,
                )
            }
            Instruction::GetArrayEl
                if array_pc >= 2
                    && comparison(array_pc + 2)
                    && matches!(
                        code.get(array_pc + 3),
                        Some(Instruction::IfTrue(_) | Instruction::IfFalse(_))
                    ) =>
            {
                ("compare_branch", array_pc - 2, array_pc + 4, true, 0)
            }
            _ => continue,
        };
        if end > code.len() {
            continue;
        }
        if regions
            .iter()
            .any(|region| (region.start as usize..region.end as usize).contains(&array_pc))
        {
            continue;
        }
        if sites.len() == MAX_PER_FUNCTION {
            omitted += 1;
            continue;
        }
        let source_offset = usize::from(matches!(family, "accumulate"));
        let reason = if opcodes
            .get(start)
            .is_some_and(|op| *op != Opcode::from_instruction(&code[start]))
        {
            "overlap"
        } else if entries[start + 1..end].iter().any(|&entry| entry) {
            "control_flow"
        } else if !direct(code.get(start + source_offset))
            || !number(code.get(start + 1 + source_offset))
        {
            "source"
        } else if matches!(family, "accumulate" | "store_product") {
            let destination = code[array_pc..end].iter().find_map(|op| match op {
                Instruction::SetLocal(index) | Instruction::SetLocalCheck(index) => Some(*index),
                _ => None,
            });
            match destination
                .and_then(|index| locals.get(index as usize).map(|local| (index, local)))
            {
                None => "destination",
                Some((_, local)) if local.kind != ClosureVariableKind::Normal || local.is_const => {
                    "binding"
                }
                Some((index, local))
                    if local.is_lexical
                        && !numeric_site_initialized(
                            code,
                            &entries,
                            &initialization,
                            start,
                            index,
                        ) =>
                {
                    "initialization"
                }
                Some(_) if !number(code.get(array_pc + 1)) => "source",
                Some(_) if !discarded => "use",
                Some(_) => "unsupported_use",
            }
        } else if !number(code.get(array_pc + 1))
            || delta_shape == 1
                && (!direct(code.get(array_pc + 2)) || !number(code.get(array_pc + 3)))
            || delta_shape == 2 && !number(code.get(array_pc + 2))
        {
            "source"
        } else if !discarded {
            "use"
        } else if delta_shape != 0 {
            "computed_delta"
        } else {
            "unsupported_use"
        };
        sites.push(RejectedNumericSite {
            pc: boundaries[start],
            source_pc: start as u32,
            family,
            reason,
            lowered_window: format!("{:?}", &code[start..end]),
        });
    }
    sites.sort_by_key(|site| site.pc);
    sites.dedup_by_key(|site| site.pc);
    (sites, omitted)
}

#[cfg(feature = "profiling")]
fn numeric_site_initialized(
    code: &[Instruction],
    entries: &[bool],
    initialization: &[Option<Vec<bool>>],
    start: usize,
    index: u16,
) -> bool {
    let block = (0..=start).rev().find(|&pc| entries[pc]).unwrap_or(0);
    let mut value = initialization[block]
        .as_deref()
        .and_then(|state| state.get(index as usize))
        .copied()
        .unwrap_or(false);
    for instruction in &code[block..start] {
        match instruction {
            Instruction::InitializeLocal(slot) if *slot == index => value = true,
            Instruction::SetLocalUninitialized(slot) if *slot == index => value = false,
            _ => {}
        }
    }
    value
}

fn is_region_opcode(opcode: Opcode) -> bool {
    matches!(
        opcode,
        Opcode::NumericArrayAccumulate
            | Opcode::NumericArrayStoreProduct
            | Opcode::NumericArrayCopyElement
            | Opcode::NumericArrayAddPreInc
            | Opcode::NumericArrayStoreAndLocal
            | Opcode::NumericArrayUpdateElement
            | Opcode::NumericArrayCompareBranch
    )
}

fn validate_region_plans(
    code: &[Instruction],
    locals: Option<&[VariableDefinition]>,
    arguments: Option<&[VariableDefinition]>,
    regions: &[NumericRegion],
    constants: &[BytecodeConstant],
) -> Result<(), ExecCodeError> {
    if regions.is_empty() {
        return Ok(());
    }
    let Some(locals) = locals else {
        return Err(ExecCodeError::InvalidTarget);
    };
    let Some(arguments) = arguments else {
        return Err(ExecCodeError::InvalidTarget);
    };
    let mut occupied_until = 0usize;
    let mut entries = HashSet::new();
    let mut block_starts = vec![false; code.len()];
    if let Some(start) = block_starts.first_mut() {
        *start = true;
    }
    for (pc, instruction) in code.iter().enumerate() {
        for operand in instruction.operand_contract().0.into_iter().flatten() {
            if let Operand::Target(target) = operand {
                entries.insert(target as usize);
                if let Some(start) = block_starts.get_mut(target as usize) {
                    *start = true;
                }
            }
        }
        if matches!(instruction, Instruction::Gosub(_)) {
            entries.insert(pc + 1);
            if let Some(start) = block_starts.get_mut(pc + 1) {
                *start = true;
            }
        }
        if instruction.control_effect().ends_block()
            && let Some(start) = block_starts.get_mut(pc + 1)
        {
            *start = true;
        }
    }
    let mut initialized = vec![false; locals.len()];
    let initialization_entries = if locals.iter().any(|local| local.is_lexical) {
        definite_initialization_entries(code, locals.len())
    } else {
        vec![None; code.len()]
    };
    let mut scanned = 0usize;
    for region in regions {
        let start = region.start as usize;
        let end = region.end as usize;
        let Some(span) = code.get(start..end) else {
            return Err(ExecCodeError::InvalidTarget);
        };
        if start >= code.len() || end >= code.len() || start < occupied_until {
            return Err(ExecCodeError::InvalidTarget);
        }
        for pc in scanned..=start {
            if block_starts[pc] {
                if let Some(state) = initialization_entries[pc].as_deref() {
                    initialized.copy_from_slice(state);
                } else {
                    initialized.fill(false);
                }
            }
            if pc == start {
                break;
            }
            match code[pc] {
                Instruction::InitializeLocal(index) => {
                    if let Some(value) = initialized.get_mut(index as usize) {
                        *value = true;
                    }
                }
                Instruction::SetLocalUninitialized(index) => {
                    if let Some(value) = initialized.get_mut(index as usize) {
                        *value = false;
                    }
                }
                _ => {}
            }
        }
        scanned = start + 1;
        if (start + 1..end).any(|pc| entries.contains(&pc))
            || !matches!(
                region.operation,
                NumericOperation::StoreElementAndLocal { .. }
            ) && (!valid_direct_source(region.array, locals, arguments, &initialized)
                || !valid_number_source(region.index, locals, arguments, &initialized))
        {
            return Err(ExecCodeError::InvalidTarget);
        }
        let shape = match region.operation {
            NumericOperation::Accumulate {
                destination,
                scale,
                checked,
            } => {
                span.len() == 9
                    && valid_numeric_destination(destination, checked, locals, &initialized)
                    && valid_number_source(scale, locals, arguments, &initialized)
                    && matches_local_read(&span[0], destination, checked)
                    && instruction_matches_direct(&span[1], region.array)
                    && instruction_matches_number(&span[2], region.index, constants)
                    && matches!(span[3], Instruction::GetArrayEl)
                    && instruction_matches_number(&span[4], scale, constants)
                    && matches!(span[5], Instruction::Mul)
                    && matches!(span[6], Instruction::Add)
                    && matches_local_write(&span[7], destination, checked)
                    && matches!(span[8], Instruction::Drop)
            }
            NumericOperation::StoreProduct {
                destination,
                scale,
                checked,
            } => {
                span.len() == 7
                    && valid_numeric_destination(destination, checked, locals, &initialized)
                    && valid_number_source(scale, locals, arguments, &initialized)
                    && instruction_matches_direct(&span[0], region.array)
                    && instruction_matches_number(&span[1], region.index, constants)
                    && matches!(span[2], Instruction::GetArrayEl)
                    && instruction_matches_number(&span[3], scale, constants)
                    && matches!(span[4], Instruction::Mul)
                    && matches_local_write(&span[5], destination, checked)
                    && matches!(span[6], Instruction::Drop)
            }
            NumericOperation::CopyElement { source } => {
                span.len() == 8
                    && valid_direct_source(source.array, locals, arguments, &initialized)
                    && valid_number_source(source.index, locals, arguments, &initialized)
                    && instruction_matches_direct(&span[0], region.array)
                    && instruction_matches_number(&span[1], region.index, constants)
                    && instruction_matches_direct(&span[2], source.array)
                    && instruction_matches_number(&span[3], source.index, constants)
                    && matches!(span[4], Instruction::GetArrayEl)
                    && matches!(span[5], Instruction::Insert3)
                    && matches!(span[6], Instruction::PutArrayEl)
                    && matches!(span[7], Instruction::Drop)
            }
            NumericOperation::AddPreInc => {
                let (slot, checked) = match region.index {
                    NumberSource::Direct(DirectSource::Local(slot)) => (slot, false),
                    NumberSource::Direct(DirectSource::CheckedLocal(slot)) => (slot, true),
                    _ => return Err(ExecCodeError::InvalidTarget),
                };
                span.len() == 6
                    && valid_numeric_destination(slot, checked, locals, &initialized)
                    && instruction_matches_direct(&span[0], region.array)
                    && matches_local_read(&span[1], slot, checked)
                    && matches!(span[2], Instruction::Inc)
                    && matches_local_write(&span[3], slot, checked)
                    && matches!(span[4], Instruction::GetArrayEl)
                    && matches!(span[5], Instruction::Add)
            }
            NumericOperation::StoreElementAndLocal {
                destination,
                checked,
            } => {
                span.len() == 4
                    && valid_numeric_destination(destination, checked, locals, &initialized)
                    && matches!(region.array, DirectSource::Local(0))
                    && matches!(region.index, NumberSource::Immediate(0))
                    && matches!(span[0], Instruction::Insert3)
                    && matches!(span[1], Instruction::PutArrayEl)
                    && matches_local_write(&span[2], destination, checked)
                    && matches!(span[3], Instruction::Drop)
            }
            NumericOperation::UpdateElement { delta } => {
                let base = instruction_matches_direct(&span[0], region.array)
                    && instruction_matches_number(&span[1], region.index, constants)
                    && matches!(span[2], Instruction::GetArrayEl3);
                base && match delta {
                    UpdateDelta::Number(source) => {
                        span.len() == 8
                            && valid_number_source(source, locals, arguments, &initialized)
                            && instruction_matches_number(&span[3], source, constants)
                            && matches!(span[4], Instruction::Add)
                            && matches!(span[5], Instruction::Insert3)
                            && matches!(span[6], Instruction::PutArrayEl)
                            && matches!(span[7], Instruction::Drop)
                    }
                    UpdateDelta::ArrayProduct(product) => {
                        span.len() == 12
                            && valid_number_source(product.scale, locals, arguments, &initialized)
                            && valid_direct_source(product.array, locals, arguments, &initialized)
                            && valid_number_source(product.index, locals, arguments, &initialized)
                            && instruction_matches_number(&span[3], product.scale, constants)
                            && instruction_matches_direct(&span[4], product.array)
                            && instruction_matches_number(&span[5], product.index, constants)
                            && matches!(span[6], Instruction::GetArrayEl)
                            && matches!(span[7], Instruction::Mul)
                            && matches!(span[8], Instruction::Add)
                            && matches!(span[9], Instruction::Insert3)
                            && matches!(span[10], Instruction::PutArrayEl)
                            && matches!(span[11], Instruction::Drop)
                    }
                }
            }
            NumericOperation::CompareBranch {
                rhs,
                comparison,
                when_true,
                target,
            } => {
                span.len() == 6
                    && valid_number_source(rhs, locals, arguments, &initialized)
                    && (target as usize <= start || target as usize >= end)
                    && (target as usize) < code.len()
                    && matches!(
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
                    && instruction_matches_direct(&span[0], region.array)
                    && instruction_matches_number(&span[1], region.index, constants)
                    && matches!(span[2], Instruction::GetArrayEl)
                    && instruction_matches_number(&span[3], rhs, constants)
                    && Opcode::from_instruction(&span[4]) == comparison
                    && (matches!(span[5], Instruction::IfTrue(value) if when_true && value == target)
                        || matches!(span[5], Instruction::IfFalse(value) if !when_true && value == target))
            }
        };
        let (inputs, outputs) = match region.operation {
            NumericOperation::AddPreInc => (1, 1),
            NumericOperation::StoreElementAndLocal { .. } => (3, 0),
            _ => (0, 0),
        };
        if !shape || !region_stack_contract(span, region.peak, inputs, outputs) {
            return Err(ExecCodeError::InvalidTarget);
        }
        occupied_until = end;
    }
    Ok(())
}

fn valid_direct_source(
    source: DirectSource,
    locals: &[VariableDefinition],
    arguments: &[VariableDefinition],
    initialized: &[bool],
) -> bool {
    match source {
        DirectSource::Local(index) | DirectSource::CheckedLocal(index) => {
            locals.get(index as usize).is_some_and(|definition| {
                definition.kind == ClosureVariableKind::Normal
                    && definition.is_lexical == matches!(source, DirectSource::CheckedLocal(_))
                    && (!definition.is_lexical
                        || initialized.get(index as usize).copied().unwrap_or(false))
            })
        }
        DirectSource::Argument(index) => (index as usize) < arguments.len(),
    }
}

fn valid_number_source(
    source: NumberSource,
    locals: &[VariableDefinition],
    arguments: &[VariableDefinition],
    initialized: &[bool],
) -> bool {
    match source {
        NumberSource::Direct(source) => valid_direct_source(source, locals, arguments, initialized),
        NumberSource::Immediate(_) | NumberSource::Constant { .. } => true,
    }
}

fn valid_numeric_destination(
    index: u16,
    checked: bool,
    locals: &[VariableDefinition],
    initialized: &[bool],
) -> bool {
    locals.get(index as usize).is_some_and(|definition| {
        definition.kind == ClosureVariableKind::Normal
            && !definition.is_const
            && definition.is_lexical == checked
            && (!checked || initialized.get(index as usize).copied().unwrap_or(false))
    })
}

fn matches_local_read(instruction: &Instruction, index: u16, checked: bool) -> bool {
    matches!(instruction, Instruction::GetLocal(value) if !checked && *value == index)
        || matches!(instruction, Instruction::GetLocalCheck(value) if checked && *value == index)
}

fn matches_local_write(instruction: &Instruction, index: u16, checked: bool) -> bool {
    matches!(instruction, Instruction::SetLocal(value) if !checked && *value == index)
        || matches!(instruction, Instruction::SetLocalCheck(value) if checked && *value == index)
}

fn region_stack_contract(
    span: &[Instruction],
    expected_peak: u16,
    input_depth: usize,
    output_depth: usize,
) -> bool {
    let mut depth = input_depth;
    let mut peak = input_depth;
    for instruction in span {
        let effect = instruction.stack_contract();
        if effect.state != StackStateEffect::Ordinary {
            return false;
        }
        let Some(next) = depth
            .checked_sub(effect.popped)
            .and_then(|depth| depth.checked_add(effect.pushed))
        else {
            return false;
        };
        depth = next;
        peak = peak.max(depth);
    }
    depth == output_depth && peak - input_depth == usize::from(expected_peak)
}

fn verify_number_operand(
    code: &ExecCode,
    source: usize,
    value: NumberSource,
) -> Result<bool, ExecCodeError> {
    let decoded = code.decode(code.boundaries[source])?;
    Ok(match value {
        NumberSource::Direct(direct) => {
            decoded.opcode == direct_opcode(direct) && decoded.operand(0) == direct_index(direct)
        }
        NumberSource::Immediate(value) => {
            decoded.opcode == Opcode::PushI32 && decoded.signed_operand(0) == value
        }
        NumberSource::Constant { index, .. } => {
            decoded.opcode == Opcode::PushConst && decoded.operand(0) == index
            // The linked constant and its range were checked before
            // encoding; the word verifier checks immutable identity.
        }
    })
}

fn verify_direct_operand(
    code: &ExecCode,
    source: usize,
    value: DirectSource,
) -> Result<bool, ExecCodeError> {
    let decoded = code.decode(code.boundaries[source])?;
    let selected = matches!(
        (direct_opcode(value), decoded.opcode),
        (Opcode::GetLocal, Opcode::DenseReadLocal) | (Opcode::GetArg, Opcode::DenseReadArg)
    );
    Ok((decoded.opcode == direct_opcode(value) || selected)
        && decoded.operand(0) == direct_index(value))
}

fn select_opcodes(
    code: &[Instruction],
    locals: Option<&[VariableDefinition]>,
    regions: &[NumericRegion],
) -> Vec<Opcode> {
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
    for region in regions {
        entries.insert(region.start as usize);
        entries.insert(region.end as usize);
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
    for region in regions {
        let start = region.start as usize;
        let end = region.end as usize;
        if matches!(
            region.operation,
            NumericOperation::UpdateElement {
                delta: UpdateDelta::ArrayProduct(_)
            }
        ) {
            // Publication inserts a distinct guard before the selected
            // ordinary first operation, keeping this schedule as fallback.
            continue;
        }
        // Region starts are entries for the other selectors, so no selected
        // operation can consume across this boundary. Preserve the exact
        // generic interval without erasing an adjacent selection.
        for pc in start..end {
            opcodes[pc] = Opcode::from_instruction(&code[pc]);
        }
        opcodes[start] = region_opcode(region.operation);
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

fn published_operands(
    code: &[Instruction],
    source: usize,
    opcode: Opcode,
    regions: &[NumericRegion],
    region_ids: &[Option<usize>],
) -> Vec<EncodedOperand> {
    if is_region_opcode(opcode) {
        let index = region_ids[source].expect("planned region has no descriptor");
        let region = &regions[index];
        return vec![
            EncodedOperand {
                bits: index as u32,
                short: index <= usize::from(u16::MAX),
                target: false,
            },
            EncodedOperand {
                bits: match region.operation {
                    NumericOperation::CompareBranch { target, .. } => target,
                    _ => region.end,
                },
                short: false,
                target: true,
            },
            EncodedOperand {
                bits: region.start + 1,
                short: false,
                target: true,
            },
        ];
    }
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
        assert!(
            code.disassemble()
                .unwrap()
                .lines()
                .any(|line| { line.split_whitespace().any(|word| word == "PushConst") })
        );
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
            ExecCode::encode_with_locals(&read, &normal, &[], &[], &[])
                .unwrap()
                .opcode_at_source(0),
            Some(Opcode::NumberLocalInc)
        );
        assert_eq!(
            ExecCode::encode_with_locals(&read, &special, &[], &[], &[])
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
            ExecCode::encode_with_locals(&update, &writable, &[], &[], &[])
                .unwrap()
                .opcode_at_source(0),
            Some(Opcode::DensePreUpdateLocal)
        );
        assert_eq!(
            ExecCode::encode_with_locals(&update, &constant, &[], &[], &[])
                .unwrap()
                .opcode_at_source(0),
            Some(Opcode::GetLocal)
        );
    }

    #[test]
    fn numeric_region_publication_rejects_invalid_descriptors_and_entries() {
        let local = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        let code = [
            Instruction::GetLocal(0),
            Instruction::GetArg(0),
            Instruction::PushI32(0),
            Instruction::GetArrayEl,
            Instruction::PushI32(2),
            Instruction::Mul,
            Instruction::Add,
            Instruction::SetLocal(0),
            Instruction::Drop,
            Instruction::ReturnUndefined,
        ];
        let region = NumericRegion {
            start: 0,
            end: 9,
            array: DirectSource::Argument(0),
            index: NumberSource::Immediate(0),
            operation: NumericOperation::Accumulate {
                destination: 0,
                scale: NumberSource::Immediate(2),
                checked: false,
            },
            peak: 3,
        };
        let published =
            ExecCode::encode_with_locals(&code, &[local], &[local], &[region], &[]).unwrap();
        assert_eq!(
            published.opcode_at_source(0),
            Some(Opcode::NumericArrayAccumulate)
        );
        let first = published.decode(0).unwrap();
        assert_eq!(published.source_pc(first.operand(1)), Some(9));
        assert_eq!(published.source_pc(first.operand(2)), Some(1));
        for source in 1..9 {
            let pc = published.exec_pc(source).unwrap();
            assert_eq!(published.source_pc(pc), Some(source));
        }
        published.words[2].set(published.exec_pc(3).unwrap());
        assert_eq!(published.verify(), Err(ExecCodeError::InvalidTarget));
        let mut malformed = region;
        malformed.peak = 2;
        assert_eq!(
            ExecCode::encode_with_locals(&code, &[local], &[local], &[malformed], &[]).unwrap_err(),
            ExecCodeError::InvalidTarget
        );
        let mut entered = vec![Instruction::Goto(4)];
        entered.extend(code);
        let shifted = NumericRegion {
            start: 1,
            end: 10,
            ..region
        };
        assert_eq!(
            ExecCode::encode_with_locals(&entered, &[local], &[local], &[shifted], &[])
                .unwrap_err(),
            ExecCodeError::InvalidTarget
        );
    }

    #[test]
    fn m2_region_publication_checks_shapes_entries_and_branch_targets() {
        let local = VariableDefinition {
            name: None,
            is_lexical: false,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        };
        let cases = [
            (
                vec![
                    Instruction::GetArg(0),
                    Instruction::PushI32(0),
                    Instruction::GetArrayEl,
                    Instruction::PushI32(2),
                    Instruction::Mul,
                    Instruction::SetLocal(0),
                    Instruction::Drop,
                    Instruction::ReturnUndefined,
                ],
                NumericOperation::StoreProduct {
                    destination: 0,
                    scale: NumberSource::Immediate(2),
                    checked: false,
                },
                2,
                Opcode::NumericArrayStoreProduct,
            ),
            (
                vec![
                    Instruction::GetArg(0),
                    Instruction::PushI32(0),
                    Instruction::GetArrayEl3,
                    Instruction::PushI32(2),
                    Instruction::Add,
                    Instruction::Insert3,
                    Instruction::PutArrayEl,
                    Instruction::Drop,
                    Instruction::ReturnUndefined,
                ],
                NumericOperation::UpdateElement {
                    delta: UpdateDelta::Number(NumberSource::Immediate(2)),
                },
                4,
                Opcode::NumericArrayUpdateElement,
            ),
            (
                vec![
                    Instruction::GetArg(0),
                    Instruction::PushI32(0),
                    Instruction::GetArrayEl3,
                    Instruction::GetLocal(0),
                    Instruction::GetArg(0),
                    Instruction::PushI32(0),
                    Instruction::GetArrayEl,
                    Instruction::Mul,
                    Instruction::Add,
                    Instruction::Insert3,
                    Instruction::PutArrayEl,
                    Instruction::Drop,
                    Instruction::ReturnUndefined,
                ],
                NumericOperation::UpdateElement {
                    delta: UpdateDelta::ArrayProduct(super::super::region::ArrayProductSource {
                        array: DirectSource::Argument(0),
                        index: NumberSource::Immediate(0),
                        scale: NumberSource::Direct(DirectSource::Local(0)),
                    }),
                },
                6,
                Opcode::NumericArrayUpdateElement,
            ),
            (
                vec![
                    Instruction::GetArg(0),
                    Instruction::GetArg(1),
                    Instruction::GetArrayEl3,
                    Instruction::GetArg(3),
                    Instruction::GetArg(2),
                    Instruction::GetArg(1),
                    Instruction::GetArrayEl,
                    Instruction::Mul,
                    Instruction::Add,
                    Instruction::Insert3,
                    Instruction::PutArrayEl,
                    Instruction::Drop,
                    Instruction::ReturnUndefined,
                ],
                NumericOperation::UpdateElement {
                    delta: UpdateDelta::ArrayProduct(super::super::region::ArrayProductSource {
                        array: DirectSource::Argument(2),
                        index: NumberSource::Direct(DirectSource::Argument(1)),
                        scale: NumberSource::Direct(DirectSource::Argument(3)),
                    }),
                },
                6,
                Opcode::NumericArrayUpdateElement,
            ),
            (
                vec![
                    Instruction::GetArg(0),
                    Instruction::PushI32(0),
                    Instruction::GetArrayEl,
                    Instruction::PushI32(2),
                    Instruction::Lt,
                    Instruction::IfFalse(7),
                    Instruction::ReturnUndefined,
                    Instruction::ReturnUndefined,
                ],
                NumericOperation::CompareBranch {
                    rhs: NumberSource::Immediate(2),
                    comparison: Opcode::Lt,
                    when_true: false,
                    target: 7,
                },
                2,
                Opcode::NumericArrayCompareBranch,
            ),
        ];
        let arguments = [local; 4];
        for (code, operation, peak, opcode) in cases {
            let region = NumericRegion {
                start: 0,
                end: (code.len()
                    - if opcode == Opcode::NumericArrayCompareBranch {
                        2
                    } else {
                        1
                    }) as u32,
                array: DirectSource::Argument(0),
                index: match operation {
                    NumericOperation::UpdateElement {
                        delta: UpdateDelta::ArrayProduct(product),
                    } => product.index,
                    _ => NumberSource::Immediate(0),
                },
                operation,
                peak,
            };
            let published =
                ExecCode::encode_with_locals(&code, &[local], &arguments, &[region], &[]).unwrap();
            assert_eq!(published.opcode_at_source(0), Some(opcode));
            published.verify().unwrap();
            if matches!(
                operation,
                NumericOperation::UpdateElement {
                    delta: UpdateDelta::ArrayProduct(_)
                }
            ) {
                let guard = published.decode(published.exec_pc(0).unwrap()).unwrap();
                let fallback = guard.operand(2);
                assert_eq!(fallback, guard.next_pc);
                assert_eq!(published.source_pc(fallback), Some(0));
                assert!(published.is_boundary(fallback as usize));
                assert_eq!(
                    published.opcode_at_exec(fallback as usize),
                    Some(Opcode::GetArg)
                );
                assert_eq!(
                    published.decode(fallback).unwrap().next_pc,
                    published.exec_pc(1).unwrap()
                );
                let ordinary =
                    ExecCode::encode_with_locals(&code, &[local], &arguments, &[], &[]).unwrap();
                for source in 1..region.end as usize {
                    assert_eq!(
                        published.opcode_at_source(source),
                        ordinary.opcode_at_source(source)
                    );
                }
                assert_eq!(
                    published.opcode_at_source(2),
                    Some(Opcode::GetArrayEl3Dense)
                );
                assert_eq!(published.opcode_at_source(6), Some(Opcode::GetArrayElDense));
                if matches!(region.index, NumberSource::Direct(_)) {
                    assert_eq!(published.opcode_at_source(4), Some(Opcode::DenseReadArg));
                }
                let fallback_target = published.words[2].get();
                published.words[2].set(published.exec_pc(1).unwrap());
                assert_eq!(published.verify(), Err(ExecCodeError::InvalidTarget));
                published.words[2].set(fallback_target);
                let fallback_header = published.words[fallback as usize].get();
                published.words[fallback as usize].set(
                    (fallback_header & !(u32::from(OPCODE_MASK) << 16))
                        | (u32::from(Opcode::GetLocal as u16) << 16),
                );
                assert_eq!(published.verify(), Err(ExecCodeError::InvalidTarget));
                published.words[fallback as usize].set(fallback_header);
                let mut malformed_payload = published.clone();
                let mut descriptors = malformed_payload.regions.as_ref().unwrap().to_vec();
                descriptors[0].value = NumberSource::Immediate(1);
                malformed_payload.regions = Some(Rc::from(descriptors));
                assert_eq!(
                    malformed_payload.verify(),
                    Err(ExecCodeError::InvalidTarget)
                );
            }
            let mut malformed = region;
            malformed.peak -= 1;
            assert_eq!(
                ExecCode::encode_with_locals(&code, &[local], &arguments, &[malformed], &[])
                    .unwrap_err(),
                ExecCodeError::InvalidTarget
            );
            let terminal = NumericRegion {
                start: code.len() as u32,
                end: code.len() as u32,
                ..region
            };
            assert_eq!(
                ExecCode::encode_with_locals(&code, &[local], &arguments, &[terminal], &[])
                    .unwrap_err(),
                ExecCodeError::InvalidTarget
            );
            let mut entered = vec![Instruction::Goto(3)];
            entered.extend(code.iter().cloned());
            let shifted = NumericRegion {
                start: 1,
                end: region.end + 1,
                ..region
            };
            assert_eq!(
                ExecCode::encode_with_locals(&entered, &[local], &arguments, &[shifted], &[])
                    .unwrap_err(),
                ExecCodeError::InvalidTarget
            );
        }
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
