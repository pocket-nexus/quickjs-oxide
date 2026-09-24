//! Live Map/Set records indexed by key and by monotonic insertion identity.
//!
//! Records alone own keys, values and GC edges. Record IDs are never reused,
//! even after clear, so paused cursors need no deleted slots or observer leases.
//! Records live in a dense slot array; a separate live index stays sorted by
//! ID (IDs are monotonic, so this is insertion order) and marks deletions with
//! a tombstone until the next geometric compaction.
//! Key lookup is average O(1); ID lookup, insertion, deletion and finding the
//! next live ID are O(log size). Heap collection transactions retain/release
//! edges around these pure mutations.

use super::collection_index::CollectionIndex;
use super::{Heap, HeapError, RawValue};

#[derive(Clone, Debug)]
pub struct MapRecord {
    pub key: RawValue,
    pub value: RawValue,
}

impl MapRecord {
    fn vacant() -> Self {
        Self {
            key: RawValue::Undefined,
            value: RawValue::Undefined,
        }
    }
}

#[derive(Clone, Debug)]
struct Slot {
    record: MapRecord,
    hash: u64,
}

#[derive(Clone, Copy, Debug)]
struct LiveEntry {
    id: usize,
    slot: u32,
}

const DEAD: u32 = u32::MAX;

#[derive(Clone, Debug, Default)]
pub struct CollectionRecords {
    slots: Vec<Slot>,
    /// Live and tombstoned entries, strictly ordered by `id`; entry count
    /// always matches `slots.len()`.
    live: Vec<LiveEntry>,
    live_len: usize,
    key_index: CollectionIndex,
    next_id: usize,
}

impl CollectionRecords {
    pub fn len(&self) -> usize {
        self.live_len
    }

    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.live_len == 0
    }

    /// Exclusive upper bound of issued IDs, independent of current live size.
    pub fn next_id(&self) -> usize {
        self.next_id
    }

    fn live_entry(&self, id: usize) -> Option<&LiveEntry> {
        let position = self.live.binary_search_by_key(&id, |entry| entry.id).ok()?;
        let entry = &self.live[position];
        (entry.slot != DEAD).then_some(entry)
    }

    pub fn get(&self, id: usize) -> Option<&MapRecord> {
        let entry = self.live_entry(id)?;
        Some(&self.slots[entry.slot as usize].record)
    }

    #[cfg(test)]
    pub(super) fn get_mut(&mut self, id: usize) -> Option<&mut MapRecord> {
        let entry = *self.live_entry(id)?;
        Some(&mut self.slots[entry.slot as usize].record)
    }

    /// Value replacement cannot invalidate either key or insertion indexes.
    pub(super) fn replace_value(&mut self, id: usize, value: RawValue) -> Option<RawValue> {
        let entry = *self.live_entry(id)?;
        Some(std::mem::replace(
            &mut self.slots[entry.slot as usize].record.value,
            value,
        ))
    }

    fn live_entries(&self) -> LiveEntries<'_> {
        LiveEntries {
            live: &self.live,
            front: 0,
            back: self.live.len(),
            remaining: self.live_len,
        }
    }

    pub fn ids(&self) -> impl DoubleEndedIterator<Item = usize> + ExactSizeIterator + '_ {
        self.live_entries().map(|entry| entry.id)
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &MapRecord> + ExactSizeIterator {
        self.live_entries()
            .map(|entry| &self.slots[entry.slot as usize].record)
    }

    pub fn next_at_or_after(&self, cursor: usize) -> Option<(usize, &MapRecord)> {
        let start = self.live.partition_point(|entry| entry.id < cursor);
        let entry = self.live[start..].iter().find(|entry| entry.slot != DEAD)?;
        Some((entry.id, &self.slots[entry.slot as usize].record))
    }

    pub(super) fn find(&self, heap: &Heap, key: &RawValue) -> Option<usize> {
        self.key_index.find(heap, self, key)
    }

    pub(super) fn find_entry<'a>(
        &'a self,
        heap: &Heap,
        key: &RawValue,
    ) -> Option<(usize, &'a MapRecord)> {
        self.key_index.find_entry(heap, self, key)
    }

    /// Must run before retaining a new record's heap edges.
    pub(super) fn preflight_insert(&self) -> Result<(), HeapError> {
        self.next_id.checked_add(1).ok_or(HeapError::Overflow {
            operation: "advancing collection record identity",
        })?;
        Ok(())
    }

    /// Compute the storage key hash before the caller borrows this record set
    /// mutably; hashing string and BigInt keys needs heap access.
    pub(super) fn precompute_insert_hash(&self, heap: &Heap, key: &RawValue) -> u64 {
        self.key_index.hash(heap, key)
    }

    /// The heap has validated the key, uniqueness and ID capacity before commit.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn insert(&mut self, heap: &Heap, record: MapRecord) -> usize {
        let hash = self.precompute_insert_hash(heap, &record.key);
        self.insert_hashed(record, hash)
    }

    /// Commit a record whose storage hash was computed before the caller
    /// borrowed this record set mutably (hashing string and BigInt keys
    /// needs heap access).
    pub(super) fn insert_hashed(&mut self, record: MapRecord, hash: u64) -> usize {
        let id = self.next_id;
        self.next_id = id
            .checked_add(1)
            .expect("collection insertion was preflighted");
        self.key_index.insert_hashed(hash, id);
        let slot = u32::try_from(self.slots.len()).expect("collection slots fit u32");
        self.slots.push(Slot { record, hash });
        self.live.push(LiveEntry { id, slot });
        self.live_len += 1;
        id
    }

    pub(super) fn remove(&mut self, id: usize) -> Option<MapRecord> {
        let position = self.live.binary_search_by_key(&id, |entry| entry.id).ok()?;
        let entry = self.live[position];
        if entry.slot == DEAD {
            return None;
        }
        let slot = entry.slot;
        let hash = self.slots[slot as usize].hash;
        let record = std::mem::replace(&mut self.slots[slot as usize].record, MapRecord::vacant());
        self.key_index.remove_hashed(hash, id);
        self.live[position].slot = DEAD;
        self.live_len -= 1;
        // Geometric compaction bounds retained slots and tombstones during
        // churn; the O(live) rebuild is amortized by the 4x growth threshold.
        if self.slots.len() > self.live_len.saturating_mul(4).saturating_add(64) {
            self.compact();
        }
        Some(record)
    }

    fn compact(&mut self) {
        let mut slots = Vec::with_capacity(self.live_len);
        for entry in &mut self.live {
            if entry.slot == DEAD {
                continue;
            }
            let old = entry.slot as usize;
            entry.slot = u32::try_from(slots.len()).expect("collection slots fit u32");
            slots.push(std::mem::replace(
                &mut self.slots[old],
                Slot {
                    record: MapRecord::vacant(),
                    hash: 0,
                },
            ));
        }
        self.slots = slots;
        self.live.retain(|entry| entry.slot != DEAD);
    }

    /// Transfer all live records in insertion order, preserving the ID clock.
    pub(super) fn take_all(&mut self) -> CollectionRecordsIntoIter {
        self.key_index.clear();
        let slots = std::mem::take(&mut self.slots);
        let live = std::mem::take(&mut self.live);
        let remaining = self.live_len;
        self.live_len = 0;
        CollectionRecordsIntoIter {
            slots,
            live,
            front: 0,
            remaining,
        }
    }

    pub(super) fn validate(&self, heap: &Heap) -> Result<(), HeapError> {
        let mut occupied = vec![false; self.slots.len()];
        let mut previous = None;
        for entry in &self.live {
            if entry.id >= self.next_id || previous.is_some_and(|previous| previous >= entry.id) {
                return Err(HeapError::Invariant(
                    "collection record IDs are not strictly ordered",
                ));
            }
            previous = Some(entry.id);
            if entry.slot == DEAD {
                continue;
            }
            let slot = entry.slot as usize;
            if slot >= self.slots.len() || occupied[slot] {
                return Err(HeapError::Invariant(
                    "collection record slots do not match live storage",
                ));
            }
            occupied[slot] = true;
        }
        let live_slots = occupied.iter().filter(|occupied| **occupied).count();
        if live_slots != self.live_len || self.live.len() != self.slots.len() {
            return Err(HeapError::Invariant(
                "collection record IDs do not match live storage",
            ));
        }
        self.key_index.validate(heap, self)
    }
}

struct LiveEntries<'a> {
    live: &'a [LiveEntry],
    front: usize,
    back: usize,
    remaining: usize,
}

impl<'a> Iterator for LiveEntries<'a> {
    type Item = &'a LiveEntry;

    fn next(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            let entry = &self.live[self.front];
            self.front += 1;
            if entry.slot != DEAD {
                self.remaining -= 1;
                return Some(entry);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl DoubleEndedIterator for LiveEntries<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.back > self.front {
            self.back -= 1;
            let entry = &self.live[self.back];
            if entry.slot != DEAD {
                self.remaining -= 1;
                return Some(entry);
            }
        }
        None
    }
}

impl ExactSizeIterator for LiveEntries<'_> {}

/// Ordered ownership transfer for clear; no record snapshot.
pub struct CollectionRecordsIntoIter {
    slots: Vec<Slot>,
    live: Vec<LiveEntry>,
    front: usize,
    remaining: usize,
}

impl Iterator for CollectionRecordsIntoIter {
    type Item = MapRecord;

    fn next(&mut self) -> Option<Self::Item> {
        while self.front < self.live.len() {
            let entry = self.live[self.front];
            self.front += 1;
            if entry.slot != DEAD {
                self.remaining -= 1;
                return Some(std::mem::replace(
                    &mut self.slots[entry.slot as usize].record,
                    MapRecord::vacant(),
                ));
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.remaining, Some(self.remaining))
    }
}

impl ExactSizeIterator for CollectionRecordsIntoIter {}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(key: i32, value: i32) -> MapRecord {
        MapRecord {
            key: RawValue::Int(key),
            value: RawValue::Int(value),
        }
    }

    #[test]
    fn collection_record_storage_reclaims_capacity_and_keeps_id_clock() {
        let heap = Heap::new();
        for peak in [64, 1024, 16384] {
            let mut records = CollectionRecords::default();
            for key in 0..peak {
                records.insert(&heap, record(key as i32, key as i32));
            }
            for id in 0..peak - 1 {
                records.remove(id).unwrap();
            }
            let (buckets, candidates) = records.key_index.retained_capacities();
            assert_eq!(records.len(), 1);
            assert!(records.live.len() <= records.len().saturating_mul(4).saturating_add(64));
            assert!(records.slots.capacity() <= 68);
            assert!(buckets <= 68);
            assert!(candidates <= 68);
            println!(
                "peak={peak} live=1 slot_capacity={} bucket_capacity={buckets} candidate_capacity={candidates}",
                records.slots.capacity()
            );
            assert_eq!(records.take_all().count(), 1);
            assert_eq!(records.slots.capacity(), 0);
            assert_eq!(records.live.capacity(), 0);
            assert_eq!(records.key_index.retained_capacities(), (0, 0));
            assert_eq!(records.insert(&heap, record(1, 2)), peak);
        }
    }

    #[test]
    fn collection_record_storage_matches_a_tombstone_reference_model() {
        let heap = Heap::new();
        let mut records = CollectionRecords::default();
        let mut model: Vec<Option<(i32, i32)>> = Vec::new();
        let mut seed = 0x8cae_7753_u64;
        let mut cursor = 0;
        for step in 0..4096 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let key = ((seed >> 32) % 31) as i32;
            let existing = model
                .iter()
                .position(|entry| entry.is_some_and(|(k, _)| k == key));
            assert_eq!(records.find(&heap, &RawValue::Int(key)), existing);
            match (seed >> 16) % 7 {
                0..=2 => {
                    if let Some(id) = existing {
                        drop(records.replace_value(id, RawValue::Int(step)).unwrap());
                        model[id] = Some((key, step));
                    } else {
                        records.preflight_insert().unwrap();
                        assert_eq!(records.insert(&heap, record(key, step)), model.len());
                        model.push(Some((key, step)));
                    }
                }
                3 => {
                    if let Some(id) = existing {
                        records.remove(id).unwrap();
                        model[id] = None;
                    }
                }
                4 => {
                    let expected = model
                        .iter()
                        .enumerate()
                        .skip(cursor)
                        .find_map(|(id, entry)| entry.map(|pair| (id, pair)));
                    let actual = records.next_at_or_after(cursor);
                    assert_eq!(actual.map(|(id, _)| id), expected.map(|(id, _)| id));
                    if let Some((id, _)) = actual {
                        cursor = id + 1;
                    }
                }
                5 => cursor = 0,
                _ => {
                    let expected = model.iter().flatten().count();
                    assert_eq!(records.take_all().count(), expected);
                    model.fill(None);
                }
            }
            records.validate(&heap).unwrap();
            assert_eq!(records.len(), model.iter().flatten().count());
            assert_eq!(records.next_id(), model.len());
            let actual = records
                .iter()
                .map(|entry| (&entry.key, &entry.value))
                .collect::<Vec<_>>();
            let expected = model
                .iter()
                .flatten()
                .map(|&(key, value)| record(key, value))
                .collect::<Vec<_>>();
            assert_eq!(actual.len(), expected.len());
            for ((actual_key, actual_value), expected) in actual.iter().zip(expected.iter()) {
                assert!(
                    crate::engine::value::collection_key::same_value_zero(
                        &heap,
                        actual_key,
                        &expected.key
                    ),
                    "key mismatch: {actual_key:?}"
                );
                assert!(
                    crate::engine::value::collection_key::same_value_zero(
                        &heap,
                        actual_value,
                        &expected.value
                    ),
                    "value mismatch: {actual_value:?}"
                );
            }
        }
    }

    #[test]
    fn collection_record_identity_exhaustion_rejects_before_mutation() {
        let records = CollectionRecords {
            next_id: usize::MAX,
            ..Default::default()
        };
        assert!(matches!(
            records.preflight_insert(),
            Err(HeapError::Overflow { .. })
        ));
        assert!(records.is_empty());
    }
}
