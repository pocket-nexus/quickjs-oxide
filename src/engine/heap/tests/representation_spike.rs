//! A4's direct NaN-payload candidate must retain current identity semantics.
//!
//! These checks grant the candidate all 52 payload bits, before charging any
//! kind/tag bits. Rejecting that optimistic direct encoding also rejects a
//! narrower direct encoding. They do not reject every possible indirection or
//! boxing design, and they make no claim about an executing 8B VM's speed.

use super::*;
use crate::engine::api::runtime::Runtime;
use crate::engine::value::bigint::JsBigInt;
use crate::engine::value::{JsValue, Value};

const PAYLOAD_BITS: u32 = 52;
const INDEX_BITS: u32 = u32::BITS;
const GENERATION_BITS: u32 = PAYLOAD_BITS - INDEX_BITS;
const PAYLOAD_MASK: u64 = (1_u64 << PAYLOAD_BITS) - 1;

fn full_identity(id: ObjectId) -> u64 {
    (u64::from(id.generation) << INDEX_BITS) | u64::from(id.index)
}

fn direct_payload(id: ObjectId) -> Option<u64> {
    let identity = full_identity(id);
    (identity <= PAYLOAD_MASK).then_some(identity)
}

fn signed_payload(value: i64) -> Option<u64> {
    let minimum = -(1_i64 << (PAYLOAD_BITS - 1));
    let maximum = (1_i64 << (PAYLOAD_BITS - 1)) - 1;
    (minimum..=maximum)
        .contains(&value)
        .then_some(value as u64 & PAYLOAD_MASK)
}

#[test]
fn a4_spike_full_handle_identity_exceeds_even_optimistic_nan_payload() {
    assert_eq!(size_of::<ObjectId>(), 8);
    assert_eq!(size_of::<StringId>(), 8);
    assert_eq!(size_of::<BigIntId>(), 8);
    let last_direct = ObjectId {
        index: u32::MAX,
        generation: (1 << GENERATION_BITS) - 1,
    };
    assert_eq!(direct_payload(last_direct), Some(PAYLOAD_MASK));
    let first_overflow = ObjectId {
        index: 0,
        generation: 1 << GENERATION_BITS,
    };
    assert_eq!(direct_payload(first_overflow), None);
    assert_eq!(
        direct_payload(ObjectId {
            index: u32::MAX,
            generation: u32::MAX,
        }),
        None
    );
}

#[test]
fn a4_spike_truncated_or_recovered_generation_must_not_revive_stale_owner() {
    let mut heap = Heap::new();
    let shape = empty_shape(&mut heap);
    let stale = leaf(&mut heap, shape);
    heap.release_object(stale).unwrap();

    // Position a genuinely vacant/reusable slot at the encoding boundary.
    // This fixture skips a million irrelevant lifetimes; allocation, release,
    // generation bump and stale checks below all use the real heap paths.
    let boundary_generation = stale.generation + (1 << GENERATION_BITS) - 1;
    let slot = &mut heap.slots[stale.index as usize];
    assert!(matches!(slot.state, SlotState::Vacant));
    slot.generation = boundary_generation;
    let boundary = leaf(&mut heap, shape);
    assert_eq!(boundary.index, stale.index);
    assert_eq!(boundary.generation, boundary_generation);
    heap.release_object(boundary).unwrap();

    let replacement = leaf(&mut heap, shape);
    assert_eq!(replacement.index, stale.index);
    assert_eq!(
        replacement.generation,
        stale.generation + (1 << GENERATION_BITS)
    );
    assert_ne!(full_identity(stale), full_identity(replacement));
    assert_eq!(
        full_identity(stale) & PAYLOAD_MASK,
        full_identity(replacement) & PAYLOAD_MASK
    );
    assert_eq!(direct_payload(replacement), None);
    assert!(matches!(
        heap.retain_object(stale),
        Err(HeapError::Stale { .. })
    ));
    assert_eq!(heap.object_strong_count(replacement), Ok(1));

    // An index-only decoder that reads the slot's current generation would
    // produce this *different* live identity from the stale input. Accepting
    // it would lose the existing checked-boundary rejection above.
    let incorrectly_recovered = ObjectId {
        index: stale.index,
        generation: heap.slots[stale.index as usize].generation,
    };
    assert_eq!(incorrectly_recovered, replacement);
    assert_ne!(incorrectly_recovered, stale);
    assert!(heap.object(incorrectly_recovered).is_ok());

    heap.release_object(replacement).unwrap();
    heap.release_shape(shape).unwrap();
    assert_eq!(heap.counts().live, 0);
}

#[test]
fn a4_spike_real_short_bigint_values_require_more_than_nan_payload() {
    let minimum = -(1_i64 << (PAYLOAD_BITS - 1));
    let maximum = (1_i64 << (PAYLOAD_BITS - 1)) - 1;
    assert!(signed_payload(minimum).is_some());
    assert!(signed_payload(maximum).is_some());
    assert_eq!(signed_payload(minimum - 1), None);
    assert_eq!(signed_payload(maximum + 1), None);

    let runtime = Runtime::new();
    // The unchanged fixed bigint64 workload starts at a=2^27. Its very first
    // a*a is 2^54: currently an immediate i64, already beyond all 52 bits.
    // Also cover the complete immediate range, not just that workload value.
    for integer in [1_i64 << 54, i64::MIN, i64::MAX] {
        let value = runtime
            .into_jsvalue(Value::BigInt(JsBigInt::from(integer)))
            .unwrap();
        assert!(matches!(value, JsValue::ShortBigInt(found) if found == integer));
        assert!(matches!(value.as_raw(), RawValue::ShortBigInt(found) if found == integer));
        assert_eq!(signed_payload(integer), None);
        runtime.release_jsvalue(value).unwrap();
    }
}
