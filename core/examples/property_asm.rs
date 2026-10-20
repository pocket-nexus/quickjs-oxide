//! Release-assembly probe for the property-hit path.
//! `cargo build --release -p quickjs-oxide-core --example property_asm`, then
//! disassemble the `probe_*` functions.
//!
//! - `probe_get_slot`: the load an inline cache performs after a shape hit,
//!   `values[start + slot]` behind the header.
//! - `probe_lookup_hit`: the uncached path for a small shape, a walk of the
//!   transition chain, then the load.

use std::hint::black_box;

use oxide_core::heap::gc::HeapLimits;
use oxide_core::{Heap, ObjectId, PropertyFlags, PropertyKey, Runtime, ShapeId, Value};

#[inline(never)]
pub fn probe_get_slot(heap: &Heap, object: ObjectId, slot: u32) -> Value {
    heap.get_slot(object, slot)
}

/// An inline-cache hit: compare the shape, then load.
#[inline(never)]
pub fn probe_cached_hit(heap: &Heap, object: ObjectId, shape: ShapeId, slot: u32) -> Option<Value> {
    let header = heap.header(object);
    if header.shape != shape {
        return None;
    }
    Some(heap.value_heap().get_at(header.slots_start + slot))
}

#[inline(never)]
pub fn probe_lookup_hit(heap: &Heap, object: ObjectId, key: PropertyKey) -> Option<Value> {
    let (slot, _) = heap.lookup_own(object, key)?;
    Some(heap.get_slot(object, slot))
}

fn main() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let object = runtime.new_object().unwrap();
    runtime.hold(Value::object(object));
    let heap = runtime.heap_mut();
    let keys: Vec<_> = ["a", "b", "c", "d"]
        .iter()
        .map(|name| heap.property_key(name).unwrap())
        .collect();
    for (index, &key) in keys.iter().enumerate() {
        heap.add_property(
            object,
            key,
            PropertyFlags::DEFAULT,
            Value::int32(index as i32),
        )
        .unwrap();
    }
    let shape = heap.shape_of(object);
    let mut sum = 0i64;
    for (slot, &key) in keys.iter().enumerate() {
        let heap = black_box(&*heap);
        sum += i64::from(
            probe_get_slot(heap, object, slot as u32)
                .as_int32()
                .unwrap(),
        );
        sum += i64::from(
            probe_cached_hit(heap, object, shape, slot as u32)
                .unwrap()
                .as_int32()
                .unwrap(),
        );
        sum += i64::from(
            probe_lookup_hit(heap, object, key)
                .unwrap()
                .as_int32()
                .unwrap(),
        );
    }
    println!("{sum}");
}
