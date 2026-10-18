//! The ordered hash table of `Map`, `Set`, `WeakMap` and `WeakSet`.
//!
//! The entries live in the record's internal slots, one or two values each
//! (`width`), in insertion order; a deleted entry's key is `Value::EMPTY`.
//! [`Table`] holds the open-addressing index over them and each entry's
//! hash. When tombstones outnumber the live entries the slots compact, and
//! every live iterator's cursor (registered as a weak reference, so an
//! abandoned iterator costs nothing once collected) is moved to the same
//! entry's new position.

use std::cell::Cell;
use std::rc::{Rc, Weak};

use super::object::InternalData;
use super::{Heap, HeapError};
use crate::value::{Kind, ObjectId, Value};

/// An empty index bucket.
const VACANT: u32 = 0;
/// A bucket whose entry was deleted.
const DELETED: u32 = u32::MAX;

/// The index and bookkeeping of one collection.
#[derive(Debug)]
pub struct Table {
    width: usize,
    /// Buckets: [`VACANT`], [`DELETED`], or an entry number plus one.
    index: Vec<u32>,
    /// Each entry's hash, live or deleted.
    hashes: Vec<u32>,
    live: usize,
    cursors: Vec<Weak<Cell<u32>>>,
}

impl Table {
    pub fn new(width: usize) -> Self {
        Table {
            width,
            index: Vec::new(),
            hashes: Vec::new(),
            live: 0,
            cursors: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.live
    }

    pub fn is_empty(&self) -> bool {
        self.live == 0
    }

    pub fn width(&self) -> usize {
        self.width
    }

    /// Whether deleted entry `entry` is a QuickJS zombie: the record a live
    /// iterator last returned, which QuickJS keeps linked (and its printer
    /// shows as an empty item) until the iterator moves on.
    pub fn is_zombie(&self, entry: usize) -> bool {
        self.cursors.iter().any(|cursor| {
            cursor
                .upgrade()
                .is_some_and(|cursor| cursor.get() as usize == entry + 1)
        })
    }

    /// Entries, live or deleted: the bound of an iteration cursor.
    pub fn entries(&self) -> usize {
        self.hashes.len()
    }

    pub fn owned_bytes(&self) -> usize {
        self.index.capacity() * 4 + self.hashes.capacity() * 4 + self.cursors.capacity() * 8
    }

    fn insert_index(&mut self, hash: u32, entry: usize) {
        let mask = self.index.len() - 1;
        let mut bucket = hash as usize & mask;
        while self.index[bucket] != VACANT && self.index[bucket] != DELETED {
            bucket = (bucket + 1) & mask;
        }
        self.index[bucket] = entry as u32 + 1;
    }

    /// Rebuilds the index with room for `entries` entries.
    fn rebuild(&mut self, entries: usize, slots: &[Value]) {
        let buckets = (entries * 2).next_power_of_two().max(8);
        self.index.clear();
        self.index.resize(buckets, VACANT);
        for entry in 0..self.hashes.len() {
            if !slots[entry * self.width].is_empty() {
                self.insert_index(self.hashes[entry], entry);
            }
        }
    }
}

/// Deletes entry `entry` of a collection record without compacting (the
/// collector's removal of a dead weak key).
pub(crate) fn tombstone(internal: &mut super::object::Internal, entry: usize) {
    let InternalData::Table(table) = &mut internal.data else {
        unreachable!("a collection");
    };
    let width = table.width;
    let hash = table.hashes[entry];
    let mask = table.index.len() - 1;
    let mut bucket = hash as usize & mask;
    while table.index[bucket] != entry as u32 + 1 {
        bucket = (bucket + 1) & mask;
    }
    table.index[bucket] = DELETED;
    table.live -= 1;
    for slot in &mut internal.slots[entry * width..entry * width + width] {
        *slot = Value::EMPTY;
    }
}

impl Heap {
    /// The hash of a key under `SameValueZero`: numbers by value (so `1` and
    /// `1.0` and `-0` and `+0` agree), strings and BigInts by content,
    /// everything else by identity.
    pub(crate) fn table_hash(&self, key: Value) -> u32 {
        let bits = match key.kind() {
            Kind::Int32 | Kind::Number => {
                let number = key.as_f64().unwrap();
                if number == 0.0 {
                    0
                } else if number.is_nan() {
                    0x7FF8_0000_0000_0000
                } else {
                    number.to_bits()
                }
            }
            Kind::String => return self.string(key.as_string().unwrap()).hash(),
            Kind::BigInt => {
                let record = self.bigint(key.as_bigint().unwrap());
                let mut hash = u64::from(record.negative);
                for &limb in record.limbs.iter() {
                    hash = hash.rotate_left(5) ^ limb;
                }
                hash
            }
            _ => key.bits(),
        };
        let mixed = bits.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        (mixed >> 32) as u32 ^ mixed as u32
    }

    pub(crate) fn table(&self, object: ObjectId) -> &Table {
        match &self.internal(object).data {
            InternalData::Table(table) => table,
            _ => unreachable!("a collection"),
        }
    }

    fn table_mut(&mut self, object: ObjectId) -> &mut Table {
        match &mut self.internal_mut(object).data {
            InternalData::Table(table) => table,
            _ => unreachable!("a collection"),
        }
    }

    /// The entry number of `key`, if present.
    pub(crate) fn table_find(&self, object: ObjectId, key: Value) -> Option<usize> {
        let table = self.table(object);
        if table.live == 0 {
            return None;
        }
        let hash = self.table_hash(key);
        let slots = &self.internal(object).slots;
        let mask = table.index.len() - 1;
        let mut bucket = hash as usize & mask;
        loop {
            match table.index[bucket] {
                VACANT => return None,
                DELETED => {}
                entry => {
                    let entry = entry as usize - 1;
                    if table.hashes[entry] == hash
                        && crate::vm::ops::same_value_zero_heap(
                            self,
                            slots[entry * table.width],
                            key,
                        )
                    {
                        return Some(entry);
                    }
                }
            }
            bucket = (bucket + 1) & mask;
        }
    }

    /// The value of a map's `key`.
    pub(crate) fn table_get(&self, object: ObjectId, key: Value) -> Option<Value> {
        let entry = self.table_find(object, key)?;
        let width = self.table(object).width;
        Some(self.internal(object).slots[entry * width + width - 1])
    }

    /// Adds `key` (with `value` for a map) or replaces its value. `-0` is
    /// stored as `+0`.
    pub(crate) fn table_set(
        &mut self,
        object: ObjectId,
        key: Value,
        value: Value,
    ) -> Result<(), HeapError> {
        let key = if key.as_f64() == Some(0.0) {
            Value::int32(0)
        } else {
            key
        };
        let width = self.table(object).width;
        if let Some(entry) = self.table_find(object, key) {
            if width == 2 {
                self.internal_mut(object).slots[entry * 2 + 1] = value;
            }
            return Ok(());
        }
        let hash = self.table_hash(key);
        self.reserve_internal_slots(object, width, &[key, value])?;
        let internal = self.internal_mut(object);
        internal.slots.push(key);
        if width == 2 {
            internal.slots.push(value);
        }
        let InternalData::Table(table) = &mut internal.data else {
            unreachable!("a collection");
        };
        let entry = table.hashes.len();
        table.hashes.push(hash);
        table.live += 1;
        // Keep the index at most half full, counting deleted buckets.
        if (entry + 1) * 2 > table.index.len() {
            let entries = table.live.max(entry + 1);
            table.rebuild(entries, &internal.slots);
        } else {
            table.insert_index(hash, entry);
        }
        Ok(())
    }

    /// Deletes `key`; whether it was present.
    pub(crate) fn table_delete(&mut self, object: ObjectId, key: Value) -> bool {
        let Some(entry) = self.table_find(object, key) else {
            return false;
        };
        self.table_delete_entry(object, entry);
        true
    }

    /// Deletes entry `entry` (live), compacting when tombstones dominate.
    pub(crate) fn table_delete_entry(&mut self, object: ObjectId, entry: usize) {
        let internal = self.internal_mut(object);
        let InternalData::Table(table) = &mut internal.data else {
            unreachable!("a collection");
        };
        let width = table.width;
        let hash = table.hashes[entry];
        let mask = table.index.len() - 1;
        let mut bucket = hash as usize & mask;
        while table.index[bucket] != entry as u32 + 1 {
            bucket = (bucket + 1) & mask;
        }
        table.index[bucket] = DELETED;
        table.live -= 1;
        for slot in &mut internal.slots[entry * width..entry * width + width] {
            *slot = Value::EMPTY;
        }
        if table.hashes.len() >= 16 && table.live * 2 < table.hashes.len() {
            self.table_compact(object);
        }
    }

    /// Deletes every entry.
    pub(crate) fn table_clear(&mut self, object: ObjectId) {
        let internal = self.internal_mut(object);
        internal.slots.fill(Value::EMPTY);
        self.table_compact(object);
    }

    /// Drops the deleted entries and moves every live cursor to the new
    /// position of the entry it was at.
    fn table_compact(&mut self, object: ObjectId) {
        let internal = self.internal_mut(object);
        let InternalData::Table(table) = &mut internal.data else {
            unreachable!("a collection");
        };
        let width = table.width;
        let entries = table.hashes.len();
        // `before[i]`: the live entries before entry `i`.
        let mut before = Vec::with_capacity(entries + 1);
        let mut kept = 0u32;
        let mut write = 0;
        for entry in 0..entries {
            before.push(kept);
            if !internal.slots[entry * width].is_empty() {
                for offset in 0..width {
                    internal.slots[write * width + offset] = internal.slots[entry * width + offset];
                }
                table.hashes[write] = table.hashes[entry];
                write += 1;
                kept += 1;
            }
        }
        before.push(kept);
        internal.slots.truncate(write * width);
        table.hashes.truncate(write);
        table.cursors.retain(|cursor| match cursor.upgrade() {
            Some(cursor) => {
                let at = (cursor.get() as usize).min(entries);
                cursor.set(before[at]);
                true
            }
            None => false,
        });
        table.live = write;
        table.rebuild(write, &internal.slots);
    }

    /// A cursor at the first entry, moved by compactions.
    pub(crate) fn table_cursor(&mut self, object: ObjectId) -> Rc<Cell<u32>> {
        let cursor = Rc::new(Cell::new(0));
        let table = self.table_mut(object);
        table.cursors.retain(|cursor| cursor.strong_count() > 0);
        table.cursors.push(Rc::downgrade(&cursor));
        cursor
    }

    /// The live entry at or after `cursor` (its key and value, the value
    /// being the key for a set), advancing the cursor past it.
    pub(crate) fn table_next(
        &self,
        object: ObjectId,
        cursor: &Cell<u32>,
    ) -> Option<(Value, Value)> {
        let table = self.table(object);
        let slots = &self.internal(object).slots;
        let width = table.width;
        let mut entry = cursor.get() as usize;
        while entry < table.hashes.len() {
            let key = slots[entry * width];
            entry += 1;
            if !key.is_empty() {
                cursor.set(entry as u32);
                return Some((key, slots[entry * width - 1]));
            }
        }
        cursor.set(entry as u32);
        None
    }

    /// Reserves room for `additional` internal slots of `object`, charging
    /// the growth to the trigger; `keep` survives the safepoint.
    pub(crate) fn reserve_internal_slots(
        &mut self,
        object: ObjectId,
        additional: usize,
        keep: &[Value],
    ) -> Result<(), HeapError> {
        let internal = self.internal(object);
        if internal.slots.capacity() - internal.slots.len() >= additional {
            return Ok(());
        }
        let extra = internal.slots.capacity().max(4) * 8;
        let mut kept = Vec::with_capacity(keep.len() + 1);
        kept.push(Value::object(object));
        kept.extend_from_slice(keep);
        self.safepoint(
            &super::Need {
                extra,
                ..Default::default()
            },
            &kept,
        )?;
        let internal = self.internal_mut(object);
        let grow = internal.slots.capacity().max(4).max(additional);
        internal.slots.reserve(grow);
        Ok(())
    }
}
