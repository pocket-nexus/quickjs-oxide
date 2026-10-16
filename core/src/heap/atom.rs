//! The atom table and `PropertyKey`.
//!
//! An atom is a string record with `is_atom` set, found by content through the
//! table. In phase 1 atoms are strong roots.
//!
//! `PropertyKey(u32)` keeps the current core's tagging: bit 31 marks an
//! immediate array index (`0 ..= 2^31 - 1`), which never enters the atom
//! table; otherwise bit 30 separates symbols from atoms. Array indices from
//! `2^31` to `2^32 - 2` are atoms whose record caches the index, so key
//! ordering still treats them as indices.

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

enum Bucket {
    One(StringId),
    Many(Vec<StringId>),
}

/// Content → atom. Buckets are keyed by the content hash and compared by
/// content against the string arena, so the table stores no second copy of
/// any string.
#[derive(Default)]
pub struct AtomTable {
    buckets: FxHashMap<u32, Bucket>,
    count: usize,
}

impl AtomTable {
    pub fn find(
        &self,
        strings: &Arena<StringRecord>,
        repr: &StringRepr,
        hash: u32,
    ) -> Option<StringId> {
        let matches = |id: &StringId| strings.get(id.0).repr == *repr;
        match self.buckets.get(&hash)? {
            Bucket::One(id) => matches(id).then_some(*id),
            Bucket::Many(ids) => ids.iter().copied().find(matches),
        }
    }

    pub fn insert(&mut self, hash: u32, id: StringId) {
        self.count += 1;
        match self.buckets.get_mut(&hash) {
            None => {
                self.buckets.insert(hash, Bucket::One(id));
            }
            Some(bucket) => match bucket {
                Bucket::One(first) => *bucket = Bucket::Many(vec![*first, id]),
                Bucket::Many(ids) => ids.push(id),
            },
        }
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn ids(&self) -> impl Iterator<Item = StringId> + '_ {
        self.buckets.values().flat_map(|bucket| match bucket {
            Bucket::One(id) => std::slice::from_ref(id).iter().copied(),
            Bucket::Many(ids) => ids.iter().copied(),
        })
    }

    /// Approximate bytes of the table itself (not the string records).
    pub fn bytes(&self) -> usize {
        let entry = size_of::<u32>() + size_of::<Bucket>() + 1;
        self.buckets.capacity() * entry
            + self
                .buckets
                .values()
                .map(|bucket| match bucket {
                    Bucket::One(_) => 0,
                    Bucket::Many(ids) => ids.capacity() * 4,
                })
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
