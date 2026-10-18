//! Inline caches: one 16-byte entry per cache site, in a store beside the code
//! store.
//!
//! The loop borrows the running unit's code from the [`CodeStore`] across every
//! heap call, so a cache cannot live in a [`CodeUnit`]. Entries live here
//! instead, indexed by the unit's `ic_base` plus the site operand of the
//! instruction. The store sits inside the [`Heap`], as the register file does,
//! so the collector prunes it at every collection without a parameter: shape,
//! object and cell indices are recycled, and an entry naming a dead record
//! would hit on a recycled id. Entries are weak.
//!
//! [`CodeStore`]: crate::bytecode::CodeStore
//! [`CodeUnit`]: crate::bytecode::CodeUnit
//! [`Heap`]: crate::heap::Heap

use crate::bytecode::{CodeUnit, Opcode};
use crate::heap::arena::Arena;
use crate::heap::object::{NONE, ObjectHeader, ObjectKind, flags};
use crate::heap::string::StringRecord;
use crate::heap::values::ValueHeap;
use crate::shape::{STATIC_CELL, ValidityCell};
use crate::value::{ShapeId, ValidityCellId, Value};

/// What a cache site caches; decides how [`IcStore::prune`] reads an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IcKind {
    /// `GetField`: `shape`, `holder`, `slot`, `cell`.
    Get,
    /// `SetField`: `shape_before`, `shape_after`, `slot`, `cell`.
    Set,
    /// `GetGlobal`, `GetGlobalOrUndefined`, `SetGlobal`: `generation`, `slot`,
    /// `flags`.
    Global,
    /// `New`: `fn_shape`, `proto_slot`, `slots`.
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
/// `w0` of a polymorphic site; `w1` indexes its row of [`POLY`] entries.
pub const POLY_TAG: u32 = NONE - 2;
/// `w0` of a megamorphic site: it saw more than [`POLY`] receiver shapes and
/// records nothing more.
pub const MEGA_TAG: u32 = NONE - 3;
/// `w0` of a `GetField` entry for `length` read on an array or a string: the
/// array's length or the string's unit count, with no shape to compare.
pub const LENGTH_TAG: u32 = NONE - 4;
/// Entries a polymorphic site probes, in order, on the out-of-line path.
pub const POLY: usize = 4;
/// Entries of the table megamorphic `GetField` sites share: a direct-mapped
/// cache keyed by receiver shape and property key, as V8's megamorphic stub
/// cache is.
pub const MEGA_ENTRIES: usize = 4096;

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

    /// Whether the entry is a monomorphic fill rather than empty, a
    /// polymorphic link or the megamorphic mark.
    #[inline]
    pub fn is_filled(&self) -> bool {
        self.w0 < LENGTH_TAG || self.w0 == STRING_RECEIVER_TAG || self.w0 == LENGTH_TAG
    }

    /// The validity cell of a filled `GetField` or `SetField` entry.
    pub fn cell(&self) -> ValidityCellId {
        ValidityCellId(self.w3)
    }
}

/// A `GetField` hit: an own hit is one shape compare and a load; a prototype or
/// string-receiver hit adds the cell test. It takes the heap's arenas rather
/// than the heap, so the loop keeps its register window borrowed across it.
#[inline(always)]
pub(crate) fn get_hit(
    objects: &Arena<ObjectHeader>,
    cells: &Arena<ValidityCell>,
    values: &ValueHeap,
    strings: &Arena<StringRecord>,
    entry: IcEntry,
    receiver: Value,
) -> Option<Value> {
    let holder = if let Some(object) = receiver.as_object() {
        let header = objects.get(object.0);
        if header.shape.0 != entry.w0 {
            if entry.w0 == LENGTH_TAG && header.kind == ObjectKind::Array {
                return Some(length_value(header.payload as usize));
            }
            return None;
        }
        if entry.w1 == NONE {
            return Some(values.get_at(header.slots_start + entry.w2));
        }
        entry.w1
    } else {
        let string = receiver.as_string()?;
        if entry.w0 == LENGTH_TAG {
            return Some(length_value(strings.get(string.index()).len()));
        }
        if entry.w0 != STRING_RECEIVER_TAG {
            return None;
        }
        entry.w1
    };
    if !cells.get(entry.w3).valid {
        return None;
    }
    Some(values.get_at(objects.get(holder).slots_start + entry.w2))
}

/// A length as a value: an `Int32` below 2^31.
#[inline(always)]
fn length_value(length: usize) -> Value {
    match i32::try_from(length) {
        Ok(length) => Value::int32(length),
        Err(_) => Value::number(length as f64),
    }
}

/// A `SetField` hit: a write to an existing own data property when the shape
/// matches; a transition when the shape matches, the chain's cell holds, the
/// slot is inside the reserved range and the object is extensible and not a
/// prototype (whose shape change must invalidate).
#[inline(always)]
pub(crate) fn set_hit(
    objects: &mut Arena<ObjectHeader>,
    cells: &Arena<ValidityCell>,
    values: &mut ValueHeap,
    entry: IcEntry,
    receiver: Value,
    value: Value,
) -> bool {
    let Some(object) = receiver.as_object() else {
        return false;
    };
    let header = objects.get_mut(object.0);
    if header.shape.0 != entry.w0 {
        return false;
    }
    if entry.w1 != entry.w0 {
        let plain = header.flags & (flags::EXTENSIBLE | flags::USED_AS_PROTOTYPE);
        if entry.w2 >= header.slots().len
            || plain != flags::EXTENSIBLE
            || !cells.get(entry.w3).valid
        {
            return false;
        }
        header.shape = ShapeId(entry.w1);
    }
    values.set_at(header.slots_start + entry.w2, value);
    true
}

/// Hit and miss counters per cache kind, kept with the `ic-stats` feature and
/// in debug builds (so tests can see hits): release builds without the feature
/// carry no counter.
#[derive(Clone, Debug, Default)]
pub struct IcStats {
    /// Monomorphic hits, by [`IcKind`].
    pub mono_hits: [u64; 4],
    /// Hits on the out-of-line probe of a polymorphic row.
    pub poly_hits: [u64; 4],
    /// Misses that filled or refilled an entry.
    pub fills: [u64; 4],
    /// Misses that did not: an accessor, a dictionary, a complete miss, a
    /// megamorphic site.
    pub unfilled: [u64; 4],
    /// Misses per entry, both kinds.
    pub site_misses: Vec<u64>,
    /// `IterStart`s that walked a pristine array by index, and the ones that
    /// ran the protocol.
    pub iter_pristine: u64,
    pub iter_generic: u64,
}

/// Every cache entry of every published unit. Append-only, like the code
/// store: unit `u`'s entries are `entries[u.ic_base..u.ic_base + u.ic_count]`.
#[derive(Default)]
pub struct IcStore {
    pub(crate) entries: Vec<IcEntry>,
    pub(crate) kinds: Vec<IcKind>,
    /// The rows of polymorphic sites, each owned by one site.
    pub(crate) poly: Vec<[IcEntry; POLY]>,
    /// The kind of each row's site.
    poly_kinds: Vec<IcKind>,
    /// The megamorphic table: `(shape, key)` and a `GetField` entry for that
    /// shape; empty until a site goes megamorphic.
    mega: Vec<(u32, u32, IcEntry)>,
    #[cfg(any(feature = "ic-stats", debug_assertions))]
    stats: IcStats,
}

impl IcStore {
    /// Appends the entries of a unit published at `ic_base`.
    pub(crate) fn reserve(&mut self, unit: &CodeUnit) {
        debug_assert_eq!(self.entries.len(), unit.ic_base as usize);
        let first = self.kinds.len();
        let len = first + unit.ic_count as usize;
        self.entries.resize(len, IcEntry::EMPTY);
        // Sites the instructions do not name (the reserved no-cache field
        // site) keep the field kind and stay empty.
        self.kinds.resize(len, IcKind::Get);
        for insn in unit.code.iter() {
            if let (Some(kind), Some(site)) = (IcKind::of(insn.op), insn.site()) {
                self.kinds[first + site as usize] = kind;
            }
        }
        #[cfg(any(feature = "ic-stats", debug_assertions))]
        self.stats.site_misses.resize(len, 0);
    }

    #[inline(always)]
    pub fn entry(&self, index: usize) -> IcEntry {
        self.entries[index]
    }

    pub fn kind(&self, index: usize) -> IcKind {
        self.kinds[index]
    }

    /// The row of a polymorphic site.
    #[inline]
    pub fn poly_row(&self, index: usize) -> Option<&[IcEntry; POLY]> {
        let entry = self.entries[index];
        (entry.w0 == POLY_TAG).then(|| &self.poly[entry.w1 as usize])
    }

    /// Records a fill of site `index`. A shape-keyed site (`GetField`,
    /// `SetField`) is monomorphic until a second key arrives, then
    /// polymorphic up to [`POLY`] keys, then megamorphic; a refill for a key
    /// it holds replaces that entry. The other kinds hold one entry. Returns
    /// whether the entry was stored.
    pub(crate) fn record(&mut self, index: usize, fill: IcEntry) -> bool {
        let kind = self.kinds[index];
        let current = self.entries[index];
        if !matches!(kind, IcKind::Get | IcKind::Set)
            || (!current.is_filled() && current.w0 != POLY_TAG)
        {
            if current.w0 == MEGA_TAG {
                return false;
            }
            self.entries[index] = fill;
            return true;
        }
        if current.is_filled() {
            if current.w0 == fill.w0 {
                self.entries[index] = fill;
            } else {
                let row = self.poly.len() as u32;
                self.poly
                    .push([current, fill, IcEntry::EMPTY, IcEntry::EMPTY]);
                self.poly_kinds.push(kind);
                self.entries[index] = IcEntry {
                    w0: POLY_TAG,
                    w1: row,
                    w2: NONE,
                    w3: NONE,
                };
            }
            return true;
        }
        let row = &mut self.poly[current.w1 as usize];
        let slot = row
            .iter()
            .position(|entry| entry.w0 == fill.w0)
            .or_else(|| row.iter().position(IcEntry::is_empty));
        match slot {
            Some(slot) => {
                row[slot] = fill;
                true
            }
            None => {
                self.entries[index] = IcEntry {
                    w0: MEGA_TAG,
                    ..IcEntry::EMPTY
                };
                false
            }
        }
    }

    #[inline]
    fn mega_index(shape: u32, key: u32) -> usize {
        let hash = shape.wrapping_mul(0x9E37_79B9) ^ key.wrapping_mul(0x85EB_CA6B);
        (hash >> 20) as usize & (MEGA_ENTRIES - 1)
    }

    /// The megamorphic table's entry for a receiver shape and a key.
    #[inline]
    pub(crate) fn mega_entry(&self, shape: u32, key: u32) -> Option<IcEntry> {
        let &(entry_shape, entry_key, entry) = self.mega.get(Self::mega_index(shape, key))?;
        (entry_shape == shape && entry_key == key).then_some(entry)
    }

    /// Records a megamorphic site's fill in the shared table.
    pub(crate) fn record_mega(&mut self, key: u32, fill: IcEntry) {
        if self.mega.is_empty() {
            self.mega
                .resize(MEGA_ENTRIES, (EMPTY_TAG, NONE, IcEntry::EMPTY));
        }
        let index = Self::mega_index(fill.w0, key);
        self.mega[index] = (fill.w0, key, fill);
    }

    /// Whether site `index` is megamorphic.
    #[inline]
    pub(crate) fn is_megamorphic(&self, index: usize) -> bool {
        self.entries[index].w0 == MEGA_TAG
    }

    /// Empties every entry of `kind` (the global generation wrapped).
    pub(crate) fn clear_kind(&mut self, kind: IcKind) {
        for (entry, &entry_kind) in self.entries.iter_mut().zip(&self.kinds) {
            if entry_kind == kind {
                *entry = IcEntry::EMPTY;
            }
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Monomorphic entries that are filled.
    pub fn filled(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.is_filled())
            .count()
    }

    /// Sites by state: monomorphic, polymorphic, megamorphic.
    pub fn site_states(&self) -> (usize, usize, usize) {
        let count = |tag: u32| self.entries.iter().filter(|entry| entry.w0 == tag).count();
        (self.filled(), count(POLY_TAG), count(MEGA_TAG))
    }

    // ---- ic-stats ----

    #[inline(always)]
    pub(crate) fn count_hit(&mut self, kind: IcKind) {
        #[cfg(any(feature = "ic-stats", debug_assertions))]
        {
            self.stats.mono_hits[kind as usize] += 1;
        }
        let _ = kind;
    }

    #[inline(always)]
    pub(crate) fn count_iteration(&mut self, pristine: bool) {
        #[cfg(any(feature = "ic-stats", debug_assertions))]
        {
            if pristine {
                self.stats.iter_pristine += 1;
            } else {
                self.stats.iter_generic += 1;
            }
        }
        let _ = pristine;
    }

    #[inline(always)]
    pub(crate) fn count_poly_hit(&mut self, kind: IcKind) {
        #[cfg(any(feature = "ic-stats", debug_assertions))]
        {
            self.stats.poly_hits[kind as usize] += 1;
        }
        let _ = kind;
    }

    #[inline(always)]
    pub(crate) fn count_miss(&mut self, index: usize, filled: bool) {
        #[cfg(any(feature = "ic-stats", debug_assertions))]
        {
            let kind = self.kinds[index] as usize;
            if filled {
                self.stats.fills[kind] += 1;
            } else {
                self.stats.unfilled[kind] += 1;
            }
            self.stats.site_misses[index] += 1;
        }
        let _ = (index, filled);
    }

    /// The counters, with the `ic-stats` feature or in a debug build.
    pub fn stats(&self) -> Option<&IcStats> {
        #[cfg(any(feature = "ic-stats", debug_assertions))]
        return Some(&self.stats);
        #[cfg(not(any(feature = "ic-stats", debug_assertions)))]
        None
    }

    // ---- collection ----

    /// Every record the filled entries name, for the stress verifier.
    pub(crate) fn named_records(&self) -> impl Iterator<Item = Record> + '_ {
        let mono = self
            .entries
            .iter()
            .zip(&self.kinds)
            .filter(|(entry, _)| entry.is_filled())
            .flat_map(|(&entry, &kind)| records(kind, entry));
        let poly = self
            .poly
            .iter()
            .zip(&self.poly_kinds)
            .flat_map(|(row, &kind)| {
                row.iter()
                    .filter(|entry| entry.is_filled())
                    .flat_map(move |&entry| records(kind, entry))
            });
        let mega = self
            .mega
            .iter()
            .filter(|(_, _, entry)| entry.is_filled())
            .flat_map(|&(_, _, entry)| records(IcKind::Get, entry));
        mono.chain(poly).chain(mega)
    }

    /// Clears every entry that names a record `live` rejects: one linear pass
    /// over the entries and the polymorphic rows, run by the collector after
    /// marking, with `live` reading the mark bits.
    pub(crate) fn prune(&mut self, mut live: impl FnMut(Record) -> bool) -> usize {
        let mut cleared = 0;
        let rows = self.poly.iter_mut().zip(&self.poly_kinds);
        let poly = rows.flat_map(|(row, kind)| row.iter_mut().map(move |entry| (entry, kind)));
        let mega = self
            .mega
            .iter_mut()
            .map(|(_, _, entry)| (entry, &IcKind::Get));
        for (entry, &kind) in self
            .entries
            .iter_mut()
            .zip(&self.kinds)
            .chain(poly)
            .chain(mega)
        {
            if entry.is_filled() && !records(kind, *entry).all(&mut live) {
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
        IcKind::Get if entry.w0 == LENGTH_TAG => {}
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
