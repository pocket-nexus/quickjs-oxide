//! Non-owning key lookup for insertion-ordered strong collections.
//!
//! Records own keys and GC edges; this index owns only hashes and record IDs.
//! The collection storage boundary maintains both together. Lookups borrow records
//! without rooting every candidate or invoking JavaScript. Public iterator cursors remain the responsibility of ordered record storage.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hasher};

use super::{CollectionRecords, HeapError, RawValue};
use crate::engine::value::collection_key;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CollectionIndex {
    buckets: HashMap<u64, Vec<usize>>,
}

impl CollectionIndex {
    fn hash(&self, key: &RawValue) -> u64 {
        let mut hasher = self.buckets.hasher().build_hasher();
        collection_key::hash(key, &mut hasher);
        hasher.finish()
    }

    pub(super) fn find(&self, records: &CollectionRecords, key: &RawValue) -> Option<usize> {
        self.buckets
            .get(&self.hash(key))?
            .iter()
            .copied()
            .find(|&index| {
                collection_key::same_value_zero(
                    &records.get(index).expect("indexed record exists").key,
                    key,
                )
            })
    }

    pub(super) fn insert(&mut self, key: &RawValue, index: usize) {
        self.buckets.entry(self.hash(key)).or_default().push(index);
    }

    pub(super) fn remove(&mut self, key: &RawValue, index: usize) {
        let hash = self.hash(key);
        let bucket = self
            .buckets
            .get_mut(&hash)
            .expect("live collection key has a hash bucket");
        let position = bucket
            .iter()
            .position(|&entry| entry == index)
            .expect("live collection record is indexed");
        bucket.swap_remove(position);
        if bucket.is_empty() {
            self.buckets.remove(&hash);
        } else if bucket.capacity() > bucket.len().saturating_mul(4).saturating_add(64) {
            bucket.shrink_to(bucket.len().saturating_mul(2).saturating_add(32));
        }
        if self.buckets.capacity() > self.buckets.len().saturating_mul(4).saturating_add(64) {
            self.buckets
                .shrink_to(self.buckets.len().saturating_mul(2).saturating_add(32));
        }
    }

    pub(super) fn clear(&mut self) {
        self.buckets.clear();
        self.buckets.shrink_to_fit();
    }

    #[cfg(test)]
    pub(super) fn retained_capacities(&self) -> (usize, usize) {
        (
            self.buckets.capacity(),
            self.buckets.values().map(Vec::capacity).sum(),
        )
    }

    /// Publication validation; never run this full scan on an ordinary lookup.
    pub(super) fn validate(&self, records: &CollectionRecords) -> Result<(), HeapError> {
        let mut seen = std::collections::HashSet::new();
        for (&hash, indices) in &self.buckets {
            if indices.is_empty() {
                return Err(HeapError::Invariant("collection index has an empty bucket"));
            }
            for &index in indices {
                let key =
                    records
                        .get(index)
                        .map(|record| &record.key)
                        .ok_or(HeapError::Invariant(
                            "collection index points outside live records",
                        ))?;
                if !seen.insert(index) || self.hash(key) != hash {
                    return Err(HeapError::Invariant(
                        "collection index does not match its records",
                    ));
                }
            }
        }
        if seen.len() != records.len() {
            return Err(HeapError::Invariant(
                "collection index is missing live records",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::heap::MapRecord;

    fn record_store(values: Vec<MapRecord>) -> CollectionRecords {
        let mut records = CollectionRecords::default();
        for record in values {
            records.insert(record);
        }
        records
    }
    use crate::engine::value::{JsString, bigint::JsBigInt};

    #[test]
    fn equal_keys_share_hash_across_number_and_string_representations() {
        let index = CollectionIndex::default();
        let pairs = [
            (RawValue::Int(0), RawValue::Float(-0.0)),
            (RawValue::Int(42), RawValue::Float(42.0)),
            (
                RawValue::Float(f64::NAN),
                RawValue::Float(f64::from_bits(0x7ff8_0000_0000_0042)),
            ),
            (
                RawValue::String(JsString::from_static("abc")),
                RawValue::String(JsString::try_from_utf16([97, 98, 99]).unwrap()),
            ),
            (
                RawValue::BigInt(
                    JsBigInt::parse_radix("123456789012345678901234567890", 10).unwrap(),
                ),
                RawValue::BigInt(
                    JsBigInt::parse_radix("123456789012345678901234567890", 10).unwrap(),
                ),
            ),
        ];
        for (left, right) in pairs {
            assert!(collection_key::same_value_zero(&left, &right));
            assert_eq!(index.hash(&left), index.hash(&right));
        }
        assert!(!collection_key::same_value_zero(
            &RawValue::Int(1),
            &RawValue::String(JsString::from_static("1"))
        ));
        assert!(!collection_key::same_value_zero(
            &RawValue::Int(1),
            &RawValue::BigInt(JsBigInt::one())
        ));
    }

    #[test]
    fn collision_candidates_are_compared_and_removal_preserves_the_others() {
        let mut index = CollectionIndex::default();
        let records = record_store(vec![
            MapRecord {
                key: RawValue::Int(1),
                value: RawValue::Undefined,
            },
            MapRecord {
                key: RawValue::Int(2),
                value: RawValue::Undefined,
            },
        ]);
        // Force a collision to exercise the bucket path deterministically,
        // without relying on the randomized hasher finding one naturally.
        let hash = index.hash(&RawValue::Int(2));
        index.buckets.insert(hash, vec![0, 1]);
        assert_eq!(index.find(&records, &RawValue::Int(2)), Some(1));
        index.remove(&RawValue::Int(2), 1);
        assert_eq!(index.find(&records, &RawValue::Int(2)), None);
        assert_eq!(index.buckets[&hash], vec![0]);
    }

    #[test]
    fn publication_rejects_missing_or_stale_index_entries() {
        let mut index = CollectionIndex::default();
        let mut records = record_store(vec![MapRecord {
            key: RawValue::Int(1),
            value: RawValue::Undefined,
        }]);
        assert!(index.validate(&records).is_err());
        index.insert(&RawValue::Int(1), 0);
        assert!(index.validate(&records).is_ok());
        records.get_mut(0).unwrap().key = RawValue::Int(2);
        assert!(index.validate(&records).is_err());
    }
}
