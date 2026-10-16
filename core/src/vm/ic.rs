//! Inline caches (new-core phase 3, section 2): one 16-byte entry per cache
//! site, in a store beside the code store.
//!
//! The loop borrows the running unit's code from the [`CodeStore`] across
//! every heap call, so a cache cannot live in a [`CodeUnit`]. Entries live
//! here instead, indexed by the unit's `ic_base` plus the site operand of the
//! instruction (section 2.1). The store sits inside the [`Heap`], as the
//! register file does, so the collector prunes it at every collection
//! without a parameter: shape, object and cell indices are recycled, and an
//! entry naming a dead record would hit on a recycled id. Entries are weak.
//!
//! [`CodeStore`]: crate::bytecode::CodeStore
//! [`CodeUnit`]: crate::bytecode::CodeUnit
//! [`Heap`]: crate::heap::Heap

use crate::bytecode::{CodeUnit, Opcode};
use crate::heap::arena::Arena;
use crate::heap::object::{NONE, ObjectHeader};
use crate::heap::values::ValueHeap;
use crate::shape::{STATIC_CELL, ValidityCell};
use crate::value::{ValidityCellId, Value};

/// What a cache site caches; decides how [`IcStore::prune`] reads an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IcKind {
    /// `GetField`: `shape`, `holder`, `slot`, `cell` (section 2.2).
    Get,
    /// `SetField`: `shape_before`, `shape_after`, `slot`, `cell` (section 2.3).
    Set,
    /// `GetGlobal`, `GetGlobalOrUndefined`, `SetGlobal`: `generation`,
    /// `slot`, `flags` (section 2.4).
    Global,
    /// `New`: `fn_shape`, `proto_slot`, `slots` (section 2.5).
    Construct,
}

impl IcKind {
    /// The kind of cache an opcode's site indexes, if it has one.
    pub const fn of(op: Opcode) -> Option<IcKind> {
        match op {
            Opcode::GetField => Some(IcKind::Get),
            Opcode::SetField => Some(IcKind::Set),
            Opcode::GetGlobal | Opcode::GetGlobalOrUndefined | Opcode::SetGlobal => {
                Some(IcKind::Global)
            }
            Opcode::New => Some(IcKind::Construct),
            _ => None,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            IcKind::Get => "get",
            IcKind::Set => "set",
            IcKind::Global => "global",
            IcKind::Construct => "construct",
        }
    }
}

/// The `c` operand of a `GetField` or `SetField` that has no cache: a unit
/// with more than 255 named-property sites caches the first 255 and gives
/// the rest this site, whose entry is reserved and never filled.
pub const FIELD_NO_SITE: u8 = u8::MAX;

/// The site of a global form past the first 65,535 of a unit: reserved and
/// never filled, as [`FIELD_NO_SITE`].
pub const GLOBAL_NO_SITE: u32 = u16::MAX as u32;

/// One cache entry, read by the inline hit paths. The meaning of the words
/// depends on the site's [`IcKind`].
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IcEntry {
    pub w0: u32,
    pub w1: u32,
    pub w2: u32,
    pub w3: u32,
}

const _: () = assert!(size_of::<IcEntry>() == 16);

/// `w0` of an empty entry: no shape has this index.
pub const EMPTY_TAG: u32 = NONE;
/// `w0` of a `GetField` entry filled for a string receiver: the property was
/// found on `%String.prototype%` or above it.
pub const STRING_RECEIVER_TAG: u32 = NONE - 1;

impl IcEntry {
    pub const EMPTY: IcEntry = IcEntry {
        w0: EMPTY_TAG,
        w1: NONE,
        w2: NONE,
        w3: NONE,
    };

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.w0 == EMPTY_TAG
    }

    /// The validity cell of a filled `GetField` or `SetField` entry.
    pub fn cell(&self) -> ValidityCellId {
        ValidityCellId(self.w3)
    }
}

/// A `GetField` hit (section 2.2): an own hit is one shape compare and a
/// load; a prototype or string-receiver hit adds the cell test. It takes the
/// heap's arenas rather than the heap, so the loop keeps its register window
/// borrowed across it.
#[inline(always)]
pub(crate) fn get_hit(
    objects: &Arena<ObjectHeader>,
    cells: &Arena<ValidityCell>,
    values: &ValueHeap,
    entry: IcEntry,
    receiver: Value,
) -> Option<Value> {
    let holder = if let Some(object) = receiver.as_object() {
        let header = objects.get(object.0);
        if header.shape.0 != entry.w0 {
            return None;
        }
        if entry.w1 == NONE {
            return Some(values.get_at(header.slots_start + entry.w2));
        }
        entry.w1
    } else if entry.w0 == STRING_RECEIVER_TAG && receiver.as_string().is_some() {
        entry.w1
    } else {
        return None;
    };
    if !cells.get(entry.w3).valid {
        return None;
    }
    Some(values.get_at(objects.get(holder).slots_start + entry.w2))
}

/// Every cache entry of every published unit. Append-only, like the code
/// store: unit `u`'s entries are `entries[u.ic_base..u.ic_base + u.ic_count]`.
#[derive(Default)]
pub struct IcStore {
    pub(crate) entries: Vec<IcEntry>,
    pub(crate) kinds: Vec<IcKind>,
}

impl IcStore {
    /// Appends the entries of a unit published at `ic_base`.
    pub(crate) fn reserve(&mut self, unit: &CodeUnit) {
        debug_assert_eq!(self.entries.len(), unit.ic_base as usize);
        let first = self.kinds.len();
        self.entries
            .resize(first + unit.ic_count as usize, IcEntry::EMPTY);
        // Sites the instructions do not name (the reserved no-cache field
        // site) keep the field kind and stay empty.
        self.kinds
            .resize(first + unit.ic_count as usize, IcKind::Get);
        for insn in unit.code.iter() {
            if let (Some(kind), Some(site)) = (IcKind::of(insn.op), insn.site()) {
                self.kinds[first + site as usize] = kind;
            }
        }
    }

    #[inline(always)]
    pub fn entry(&self, index: usize) -> IcEntry {
        self.entries[index]
    }

    #[inline]
    pub(crate) fn set(&mut self, index: usize, entry: IcEntry) {
        self.entries[index] = entry;
    }

    pub fn kind(&self, index: usize) -> IcKind {
        self.kinds[index]
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Filled entries.
    pub fn filled(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| !entry.is_empty())
            .count()
    }

    /// Every record the filled entries name, for the stress verifier.
    pub(crate) fn named_records(&self) -> impl Iterator<Item = Record> + '_ {
        self.entries
            .iter()
            .zip(&self.kinds)
            .filter(|(entry, _)| !entry.is_empty())
            .flat_map(|(&entry, &kind)| records(kind, entry))
    }

    /// Clears every entry that names a record `live` rejects: one linear pass,
    /// run by the collector after marking, with `live` reading the mark bits.
    pub(crate) fn prune(&mut self, mut live: impl FnMut(Record) -> bool) -> usize {
        let mut cleared = 0;
        for (entry, &kind) in self.entries.iter_mut().zip(&self.kinds) {
            if entry.is_empty() {
                continue;
            }
            if !records(kind, *entry).all(&mut live) {
                *entry = IcEntry::EMPTY;
                cleared += 1;
            }
        }
        cleared
    }
}

/// A heap record an entry names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Record {
    Shape(u32),
    Object(u32),
    Cell(u32),
}

/// The records a filled entry of `kind` names.
fn records(kind: IcKind, entry: IcEntry) -> impl Iterator<Item = Record> {
    let mut found = [None; 3];
    match kind {
        IcKind::Get => {
            if entry.w0 != STRING_RECEIVER_TAG {
                found[0] = Some(Record::Shape(entry.w0));
            }
            if entry.w1 != NONE {
                found[1] = Some(Record::Object(entry.w1));
            }
            if entry.w3 != NONE && entry.w3 != STATIC_CELL.index() {
                found[2] = Some(Record::Cell(entry.w3));
            }
        }
        IcKind::Set => {
            found[0] = Some(Record::Shape(entry.w0));
            found[1] = Some(Record::Shape(entry.w1));
            if entry.w3 != NONE && entry.w3 != STATIC_CELL.index() {
                found[2] = Some(Record::Cell(entry.w3));
            }
        }
        IcKind::Global => {}
        IcKind::Construct => found[0] = Some(Record::Shape(entry.w0)),
    }
    found.into_iter().flatten()
}
