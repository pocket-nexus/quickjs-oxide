//! The atom table and `PropertyKey`.
//!
//! An atom is a string record with `is_atom` set, found by content through the
//! table. An atom lives as long as something holds it, like any string: a
//! shape, a dictionary, a live dynamic unit's key list, a value. The atoms
//! interned while the runtime is created or before the program runs are
//! pinned and live as long as the heap ([`AtomMode`](super::AtomMode)); a
//! swept atom leaves the table.
//!
//! `PropertyKey(u32)` is tagged: bit 31 marks an immediate array index
//! (`0 ..= 2^31 - 1`), which never enters the atom table; otherwise bit 30
//! separates symbols from atoms. Array indices from `2^31` to `2^32 - 2` are
//! atoms whose record caches the index, so key ordering still treats them as
//! indices.

use std::fmt;

use super::arena::Arena;
use super::hash::FxHashMap;
use super::string::{StringRecord, StringRepr};
use crate::value::{StringId, SymbolId, Value};

const INDEX_TAG: u32 = 1 << 31;
const SYMBOL_TAG: u32 = 1 << 30;
/// Exclusive bound on string and symbol indices that fit in a key.
pub const KEY_INDEX_LIMIT: u32 = SYMBOL_TAG;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PropertyKey(u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyKind {
    Index(u32),
    String(StringId),
    Symbol(SymbolId),
}

impl PropertyKey {
    /// An immediate index key, for `index < 2^31`.
    #[inline]
    pub const fn index(index: u32) -> Option<PropertyKey> {
        if index < INDEX_TAG {
            Some(PropertyKey(index | INDEX_TAG))
        } else {
            None
        }
    }

    /// The key of an atom. Panics if the atom index does not fit (callers
    /// allocate atoms below [`KEY_INDEX_LIMIT`]).
    #[inline]
    pub fn atom(id: StringId) -> PropertyKey {
        assert!(id.0 < KEY_INDEX_LIMIT, "atom index out of key range");
        PropertyKey(id.0)
    }

    #[inline]
    pub fn symbol(id: SymbolId) -> PropertyKey {
        assert!(id.0 < KEY_INDEX_LIMIT, "symbol index out of key range");
        PropertyKey(id.0 | SYMBOL_TAG)
    }

    #[inline]
    pub const fn kind(self) -> KeyKind {
        if self.0 & INDEX_TAG != 0 {
            KeyKind::Index(self.0 & !INDEX_TAG)
        } else if self.0 & SYMBOL_TAG != 0 {
            KeyKind::Symbol(SymbolId(self.0 & !SYMBOL_TAG))
        } else {
            KeyKind::String(StringId(self.0))
        }
    }

    #[inline]
    pub const fn raw(self) -> u32 {
        self.0
    }

    /// The key whose [`raw`](Self::raw) bits are `bits`: an instruction's
    /// `imm` field. Every bit pattern decodes to some key.
    #[inline]
    pub const fn from_raw(bits: u32) -> PropertyKey {
        PropertyKey(bits)
    }

    /// The key as a value: an Int32 for an index, else the string or symbol.
    #[inline]
    pub const fn to_value(self) -> Value {
        match self.kind() {
            KeyKind::Index(index) => Value::int32(index as i32),
            KeyKind::String(id) => Value::string(id),
            KeyKind::Symbol(id) => Value::symbol(id),
        }
    }

    /// The heap value the key keeps alive, if any.
    #[inline]
    pub const fn heap_value(self) -> Option<Value> {
        match self.kind() {
            KeyKind::Index(_) => None,
            KeyKind::String(id) => Some(Value::string(id)),
            KeyKind::Symbol(id) => Some(Value::symbol(id)),
        }
    }
}

impl fmt::Debug for PropertyKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind() {
            KeyKind::Index(index) => write!(f, "Key::Index({index})"),
            KeyKind::String(id) => write!(f, "Key::Atom({})", id.0),
            KeyKind::Symbol(id) => write!(f, "Key::Symbol({})", id.0),
        }
    }
}

/// A bucket value: an atom index, or with [`OVERFLOW`] set an index into the
/// collision lists. Atom indices stay below [`KEY_INDEX_LIMIT`], so the tag bit
/// is free.
const OVERFLOW: u32 = 1 << 31;

/// Content → atom. Buckets are keyed by the content hash and compared by
/// content against the string arena, so the table stores no second copy of any
/// string; an entry is 8 bytes.
#[derive(Default)]
pub struct AtomTable {
    buckets: FxHashMap<u32, u32>,
    collisions: Vec<Vec<StringId>>,
    /// Collision lists no bucket uses.
    free_collisions: Vec<u32>,
    count: usize,
}

impl AtomTable {
    pub fn find(
        &self,
        strings: &Arena<StringRecord>,
        repr: &StringRepr,
        hash: u32,
    ) -> Option<StringId> {
        self.find_by(hash, |id| strings.get(id.0).flat() == Some(repr))
    }

    /// [`Self::find`] for the content of a one-byte representation.
    pub fn find_one_byte(
        &self,
        strings: &Arena<StringRecord>,
        bytes: &[u8],
        hash: u32,
    ) -> Option<StringId> {
        self.find_by(hash, |id| {
            matches!(strings.get(id.0).flat(), Some(StringRepr::OneByte(atom)) if **atom == *bytes)
        })
    }

    fn find_by(&self, hash: u32, matches: impl Fn(&StringId) -> bool) -> Option<StringId> {
        let bucket = *self.buckets.get(&hash)?;
        if bucket & OVERFLOW == 0 {
            let id = StringId(bucket);
            matches(&id).then_some(id)
        } else {
            self.collisions[(bucket & !OVERFLOW) as usize]
                .iter()
                .copied()
                .find(matches)
        }
    }

    pub fn insert(&mut self, hash: u32, id: StringId) {
        debug_assert!(id.0 < KEY_INDEX_LIMIT);
        self.count += 1;
        match self.buckets.get_mut(&hash) {
            None => {
                self.buckets.insert(hash, id.0);
            }
            Some(bucket) if *bucket & OVERFLOW == 0 => {
                let ids = vec![StringId(*bucket), id];
                let list = match self.free_collisions.pop() {
                    Some(list) => {
                        self.collisions[list as usize] = ids;
                        list
                    }
                    None => {
                        self.collisions.push(ids);
                        self.collisions.len() as u32 - 1
                    }
                };
                *bucket = list | OVERFLOW;
            }
            Some(bucket) => self.collisions[(*bucket & !OVERFLOW) as usize].push(id),
        }
    }

    /// Removes atom `id`, whose content hashes to `hash` (it is being
    /// swept).
    pub fn remove(&mut self, hash: u32, id: StringId) {
        let Some(bucket) = self.buckets.get_mut(&hash) else {
            debug_assert!(false, "an atom outside the table");
            return;
        };
        if *bucket & OVERFLOW == 0 {
            debug_assert_eq!(*bucket, id.0);
            self.buckets.remove(&hash);
        } else {
            let list = *bucket & !OVERFLOW;
            let ids = &mut self.collisions[list as usize];
            let Some(position) = ids.iter().position(|&other| other == id) else {
                debug_assert!(false, "an atom outside the table");
                return;
            };
            ids.swap_remove(position);
            if let [last] = ids[..] {
                *bucket = last.0;
                self.collisions[list as usize] = Vec::new();
                self.free_collisions.push(list);
            }
        }
        self.count -= 1;
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn ids(&self) -> impl Iterator<Item = StringId> + '_ {
        self.buckets.values().flat_map(move |&bucket| {
            let ids: &[StringId] = if bucket & OVERFLOW == 0 {
                &[]
            } else {
                &self.collisions[(bucket & !OVERFLOW) as usize]
            };
            let single = (bucket & OVERFLOW == 0).then_some(StringId(bucket));
            single.into_iter().chain(ids.iter().copied())
        })
    }

    /// Approximate bytes of the table itself (not the string records).
    pub fn bytes(&self) -> usize {
        self.buckets.capacity() * (size_of::<(u32, u32)>() + 1)
            + self
                .collisions
                .iter()
                .map(|ids| 24 + ids.capacity() * 4)
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_tags_round_trip() {
        assert_eq!(PropertyKey::index(0).unwrap().kind(), KeyKind::Index(0));
        let max = INDEX_TAG - 1;
        assert_eq!(PropertyKey::index(max).unwrap().kind(), KeyKind::Index(max));
        assert_eq!(PropertyKey::index(INDEX_TAG), None);
        assert_eq!(
            PropertyKey::atom(StringId(5)).kind(),
            KeyKind::String(StringId(5))
        );
        assert_eq!(
            PropertyKey::symbol(SymbolId(5)).kind(),
            KeyKind::Symbol(SymbolId(5))
        );
        assert_ne!(
            PropertyKey::atom(StringId(5)),
            PropertyKey::symbol(SymbolId(5))
        );
        assert_eq!(PropertyKey::index(7).unwrap().to_value(), Value::int32(7));
        assert_eq!(PropertyKey::index(7).unwrap().heap_value(), None);
    }
}
