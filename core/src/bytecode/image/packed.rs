//! Indexable instructions stored in the immutable image. The common record
//! has an opcode and three bytes for its operands. Wide records are in a
//! side table. Both use the same instruction indices as source-compiled code.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    cell::{Cell, OnceCell},
    ops::Range,
};

use crate::{HeapError, Insn, KeyKind, Opcode, PropertyKey, StringId};

use super::{ImageError, Reader, Writer};

const WIDE: u8 = 255;
pub(super) const NO_IMMEDIATE: u8 = 8;
const MASK_COUNT: usize = 256;
pub(super) const MASKS: usize = MASK_COUNT;
const _: () = assert!(Opcode::ALL.len() < WIDE as usize);

/// Immutable, shareable bytes of an image inside its package allocation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageBytes(Storage);

#[derive(Clone, Debug, PartialEq, Eq)]
enum Storage {
    Shared(Rc<Box<[u8]>>, Range<usize>),
    Static(&'static [u8]),
}

impl ImageBytes {
    /// Retains a checked range of a package without copying its bytes.
    pub fn new(owner: Rc<Box<[u8]>>, range: Range<usize>) -> Option<Self> {
        owner.get(range.clone())?;
        Some(Self(Storage::Shared(owner, range)))
    }
}

impl From<&'static [u8]> for ImageBytes {
    fn from(bytes: &'static [u8]) -> Self {
        Self(Storage::Static(bytes))
    }
}

impl From<Box<[u8]>> for ImageBytes {
    fn from(bytes: Box<[u8]>) -> Self {
        let len = bytes.len();
        Self(Storage::Shared(Rc::new(bytes), 0..len))
    }
}

impl AsRef<[u8]> for ImageBytes {
    fn as_ref(&self) -> &[u8] {
        match &self.0 {
            Storage::Shared(owner, range) => &owner[range.clone()],
            Storage::Static(bytes) => bytes,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PackedRanges {
    words: Range<usize>,
    wide: Range<usize>,
    widths: [u8; 3],
}

impl PackedRanges {
    pub(super) fn of(image: &[u8], reader: &mut Reader<'_>) -> Result<Self, ImageError> {
        let encoded = reader.u16()?;
        let widths = [
            encoded as u8 & 15,
            (encoded >> 4) as u8 & 15,
            (encoded >> 8) as u8,
        ];
        if widths.iter().any(|&width| width > 8) {
            return Err(ImageError::Malformed);
        }
        let len = reader.count(4)?;
        let words = span(image, reader.take(len * 4)?);
        let count = reader.count(8)?;
        let wide = span(image, reader.take(count * 8)?);
        Ok(Self {
            words,
            wide,
            widths,
        })
    }

    pub(super) fn view<'a>(&self, image: &'a [u8], masks: &'a [u8]) -> Packed<'a> {
        Packed {
            words: &image[self.words.clone()],
            wide: &image[self.wide.clone()],
            masks,
            widths: self.widths,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.words.len() / 4
    }

    pub(super) fn bytes(&self) -> usize {
        self.words.len() + self.wide.len()
    }
}

pub(super) fn span(parent: &[u8], slice: &[u8]) -> Range<usize> {
    let offset = slice.as_ptr().addr() - parent.as_ptr().addr();
    offset..offset + slice.len()
}

#[derive(Clone, Copy)]
pub(super) struct Packed<'a> {
    words: &'a [u8],
    wide: &'a [u8],
    masks: &'a [u8],
    widths: [u8; 3],
}

impl Packed<'_> {
    #[inline]
    pub(super) fn len(self) -> usize {
        self.words.len() / 4
    }

    #[inline]
    pub(super) fn get(self, pc: usize) -> Option<Insn> {
        let &[op, lo, mid, hi] = self.words.as_chunks::<4>().0.get(pc)?;
        if op == WIDE {
            let index = u32::from_le_bytes([lo, mid, hi, 0]) as usize;
            let &[op, a, b, c, p, q, r, s] = self.wide.as_chunks::<8>().0.get(index)?;
            let op = *Opcode::ALL.get(op as usize)?;
            return Some(Insn::new(op, a, b, c, u32::from_le_bytes([p, q, r, s])));
        }
        let kind = *self.masks.get(op as usize)?;
        let op = *Opcode::ALL.get(op as usize)?;
        if kind & NO_IMMEDIATE != 0 {
            return Some(Insn::new(op, lo, mid, hi, 0));
        }
        let mut payload = u32::from_le_bytes([lo, mid, hi, 0]);
        let mut take = |bit, width| {
            let width = if kind & bit != 0 { width } else { 0 };
            let byte = payload & ((1 << width) - 1);
            payload >>= width;
            byte as u8
        };
        let [wa, wb, wc] = self.widths;
        let a = take(1, wa);
        let b = take(2, wb);
        let c = take(4, wc);
        Some(Insn::new(op, a, b, c, payload))
    }
}

pub(super) fn mode(insn: Insn) -> u8 {
    u8::from(insn.a != 0) | (u8::from(insn.b != 0) << 1) | (u8::from(insn.c != 0) << 2)
}

pub(super) fn write(
    writer: &mut Writer,
    masks: &[u8; MASK_COUNT],
    widths: [u8; 3],
    insns: impl ExactSizeIterator<Item = Result<Insn, ImageError>>,
) -> Result<(), ImageError> {
    let [wa, wb, wc] = widths.map(u16::from);
    writer.u16(wa | wb << 4 | wc << 8);
    writer.count(insns.len())?;
    let mut wide = Vec::new();
    for insn in insns {
        let insn = insn?;
        let kind = masks[insn.op as usize];
        if kind & NO_IMMEDIATE != 0 {
            debug_assert_eq!(insn.imm, 0);
            writer.0.extend([insn.op as u8, insn.a, insn.b, insn.c]);
            continue;
        }
        let [wa, wb, wc] = widths;
        let fields = [(4, insn.c, wc), (2, insn.b, wb), (1, insn.a, wa)];
        let bits: u8 = fields
            .iter()
            .filter(|(bit, _, _)| kind & bit != 0)
            .map(|(_, _, width)| *width)
            .sum();
        if insn.imm < 1 << (24 - bits) {
            let mut payload = insn.imm;
            for (bit, byte, width) in fields {
                if kind & bit != 0 {
                    payload = (payload << width) | u32::from(byte);
                }
            }
            let [lo, mid, hi, _] = payload.to_le_bytes();
            writer.0.extend([insn.op as u8, lo, mid, hi]);
        } else {
            let index = wide.len() / 8;
            if index >= 1 << 24 {
                return Err(ImageError::TooLarge);
            }
            let [lo, mid, hi, _] = (index as u32).to_le_bytes();
            writer.0.extend([WIDE, lo, mid, hi]);
            wide.extend([insn.op as u8, insn.a, insn.b, insn.c]);
            wide.extend(insn.imm.to_le_bytes());
        }
    }
    writer.count(wide.len() / 8)?;
    writer.0.extend(wide);
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ImageBacking {
    pub(super) bytes: ImageBytes,
    pub(super) masks: Range<usize>,
    pub(super) atoms: Rc<[StringId]>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Heat {
    #[default]
    Cold,
    Seen,
    // The runtime's remaining budget only decreases.
    KeepPacked,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ImageCode {
    pub(super) backing: Rc<ImageBacking>,
    pub(super) packed: PackedRanges,
    pub(super) positions: Range<usize>,
    pub(crate) cache_base: Cell<Option<u32>>,
    pub(super) decoded: OnceCell<Box<[Insn]>>,
    pub(super) heat: Cell<Heat>,
}

impl ImageCode {
    #[inline]
    pub(crate) fn get(&self, pc: usize) -> Option<Insn> {
        match self.decoded.get() {
            Some(code) => code.get(pc).copied(),
            None => self.execution().get(pc),
        }
    }

    pub(crate) fn decoded(&self) -> Option<&[Insn]> {
        self.decoded.get().map(AsRef::as_ref)
    }

    pub(crate) fn enter(&self, remaining: &Cell<usize>) -> bool {
        match self.heat.get() {
            Heat::Cold => {
                self.heat.set(Heat::Seen);
                false
            }
            Heat::Seen => self.promote(remaining),
            Heat::KeepPacked => false,
        }
    }

    pub(crate) fn promote(&self, remaining: &Cell<usize>) -> bool {
        if self.decoded.get().is_some() {
            return true;
        }
        let len = self.len();
        if self.heat.get() == Heat::KeepPacked || len > remaining.get() / size_of::<Insn>() {
            self.heat.set(Heat::KeepPacked);
            return false;
        }
        let left = remaining.get() - len * size_of::<Insn>();
        if self.decode().is_err() {
            self.heat.set(Heat::KeepPacked);
            return false;
        }
        remaining.set(left);
        true
    }

    pub(super) fn decode(&self) -> Result<(), HeapError> {
        let len = self.len();
        let mut code = Vec::new();
        code.try_reserve_exact(len)
            .map_err(|_| HeapError::OutOfMemory)?;
        let view = self.execution();
        code.extend((0..len).map(|pc| view.get(pc).expect("a validated instruction")));
        self.decoded
            .set(code.into_boxed_slice())
            .expect("image instructions decoded once");
        Ok(())
    }

    #[inline]
    pub(crate) fn execution(&self) -> Execution<'_> {
        let bytes = self.backing.bytes.as_ref();
        Execution {
            bits: self.packed.view(bytes, &bytes[self.backing.masks.clone()]),
            atoms: &self.backing.atoms,
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.packed.words.len() / 4
    }

    pub(crate) fn instruction_bytes(&self) -> usize {
        self.packed.bytes() + self.decoded().map_or(0, core::mem::size_of_val)
    }

    pub(crate) fn position_bytes(&self) -> usize {
        self.positions.len()
    }

    pub(crate) fn position(&self, pc: u32, definition: Option<(u32, u32)>) -> Option<(u32, u32)> {
        let mut reader = Reader(&self.backing.bytes.as_ref()[self.positions.clone()]);
        let mut positions = super::wire::Positions::new(definition.unwrap_or_default());
        let mut result = None;
        while !reader.0.is_empty() {
            let position = positions
                .next(&mut reader)
                .expect("an admitted position table");
            if position.pc > pc {
                break;
            }
            result = Some((position.line, position.column));
        }
        result
    }
}

pub(crate) struct Execution<'a> {
    bits: Packed<'a>,
    atoms: &'a [StringId],
}

impl Execution<'_> {
    #[inline(always)]
    pub(crate) fn get(&self, pc: usize) -> Option<Insn> {
        let mut insn = self.bits.get(pc)?;
        if super::keyed(insn.op)
            && let KeyKind::String(id) = insn.key().kind()
        {
            insn.imm = PropertyKey::atom(self.atoms[id.0 as usize]).raw();
        }
        Some(insn)
    }
}
