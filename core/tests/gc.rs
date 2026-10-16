//! M2 acceptance: collection, roots, handles, free-list reuse, the heap cap.
//!
//! Every test follows the rooting rule, so the suite also passes under
//! `--features gc-stress`, where each allocation collects first.

use oxide_core::heap::gc::HeapLimits;
use oxide_core::{Heap, HeapError, ObjectId, ObjectKind, PropertyFlags, PropertyKey, Value};

fn key(heap: &mut Heap, name: &str) -> PropertyKey {
    heap.property_key(name).unwrap()
}

/// An ordinary object held by a handle, so it survives later allocations.
fn held_object(heap: &mut Heap) -> ObjectId {
    let object = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    heap.handles().create(Value::object(object));
    object
}

#[test]
fn unreachable_cycle_is_collected() {
    let mut heap = Heap::default();
    let next = key(&mut heap, "next");
    let mark = heap.roots().mark();
    let a = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    heap.roots().push(Value::object(a));
    let b = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    heap.roots().push(Value::object(b));
    heap.add_property(a, next, PropertyFlags::DEFAULT, Value::object(b))
        .unwrap();
    heap.add_property(b, next, PropertyFlags::DEFAULT, Value::object(a))
        .unwrap();
    heap.collect();
    assert!(heap.is_live(Value::object(a)) && heap.is_live(Value::object(b)));
    heap.roots().truncate(mark);
    heap.collect();
    assert!(!heap.is_live(Value::object(a)));
    assert!(!heap.is_live(Value::object(b)));
    assert_eq!(heap.live_objects(), 0);
}

#[test]
fn rooted_values_survive_and_reachable_children_follow() {
    let mut heap = Heap::default();
    let child_key = key(&mut heap, "child");
    let mark = heap.roots().mark();
    let parent = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    heap.roots().push(Value::object(parent));
    let child = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    heap.add_property(
        parent,
        child_key,
        PropertyFlags::DEFAULT,
        Value::object(child),
    )
    .unwrap();
    let text = heap.alloc_string("payload").unwrap();
    heap.set_element(child, 0, Value::string(text)).unwrap();
    for _ in 0..3 {
        heap.collect();
    }
    assert!(heap.is_live(Value::object(parent)));
    assert!(heap.is_live(Value::object(child)));
    assert_eq!(heap.string_text(text), "payload");
    assert_eq!(
        heap.get_property(parent, child_key),
        Some(Value::object(child))
    );
    heap.roots().truncate(mark);
    heap.collect();
    assert!(!heap.is_live(Value::string(text)));
}

#[test]
fn released_handle_value_is_collected() {
    let mut heap = Heap::default();
    let object = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    let handle = heap.handles().create(Value::object(object));
    heap.collect();
    assert!(heap.is_live(Value::object(object)));
    assert_eq!(heap.handles().get(handle), Value::object(object));
    heap.handles().release(handle);
    heap.collect();
    assert!(!heap.is_live(Value::object(object)));
    assert_eq!(heap.handles().live_count(), 0);
}

#[test]
fn free_list_reuse_is_deterministic() {
    fn run() -> Vec<u32> {
        let mut heap = Heap::default();
        let mark = heap.roots().mark();
        let mut objects = Vec::new();
        for _ in 0..8 {
            let object = heap.alloc_object(ObjectKind::Ordinary, None, 2).unwrap();
            heap.roots().push(Value::object(object));
            objects.push(Value::object(object));
        }
        // Keep the odd ones.
        heap.roots().truncate(mark);
        for &object in objects.iter().skip(1).step_by(2) {
            heap.roots().push(object);
        }
        heap.collect();
        let mut reused = Vec::new();
        for _ in 0..4 {
            let object = heap.alloc_object(ObjectKind::Ordinary, None, 2).unwrap();
            heap.roots().push(Value::object(object));
            reused.push(object.index());
        }
        reused
    }
    let first = run();
    assert_eq!(first, run());
    // The freed even indices come back lowest first.
    assert_eq!(first, [0, 2, 4, 6]);
}

#[test]
fn shapes_and_transitions_are_weak() {
    let mut heap = Heap::default();
    let x = key(&mut heap, "x");
    let object = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    let handle = heap.handles().create(Value::object(object));
    heap.add_property(object, x, PropertyFlags::DEFAULT, Value::int32(1))
        .unwrap();
    let shapes = heap.live_shapes();
    heap.collect();
    assert_eq!(heap.live_shapes(), shapes);
    heap.handles().release(handle);
    heap.collect();
    assert_eq!(heap.live_shapes(), 0);
    // A new object rebuilds the tree from scratch.
    let again = held_object(&mut heap);
    heap.add_property(again, x, PropertyFlags::DEFAULT, Value::int32(2))
        .unwrap();
    assert_eq!(heap.get_property(again, x), Some(Value::int32(2)));
}

#[test]
fn scopes_and_functions_trace_their_captures() {
    let mut heap = Heap::default();
    let mark = heap.roots().mark();
    let outer = heap.alloc_scope(None, 2).unwrap();
    heap.roots().push(Value::object(outer.object()));
    let text = heap.alloc_string("captured").unwrap();
    heap.scope_set(outer, 0, 1, Value::string(text));
    let inner = heap.alloc_scope(Some(outer), 1).unwrap();
    heap.roots().truncate(mark);
    // `inner` is unrooted for this one call; the allocation roots its argument.
    let function = heap.alloc_function(None, Some(inner)).unwrap();
    let handle = heap.handles().create(Value::object(function));
    heap.collect();
    assert_eq!(heap.function_scope(function), Some(inner));
    assert_eq!(heap.scope_parent(inner), Some(outer));
    assert_eq!(heap.scope_get(inner, 1, 1), Value::string(text));
    assert!(
        heap.scope_get(inner, 0, 0).is_empty(),
        "uninitialized binding"
    );
    heap.handles().release(handle);
    heap.collect();
    assert_eq!(heap.live_objects(), 0);
    assert!(!heap.is_live(Value::string(text)));
    assert_eq!(heap.value_heap().used_slots(), 0);
}

#[test]
fn heap_cap_returns_out_of_memory() {
    let limit = 256 << 10;
    let mut heap = Heap::new(HeapLimits { max_bytes: limit });
    let array = heap.alloc_array(None, 0).unwrap();
    heap.handles().create(Value::object(array));
    let mut result = Ok(());
    for index in 0..1_000_000 {
        let object = match heap.alloc_object(ObjectKind::Ordinary, None, 4) {
            Ok(object) => object,
            Err(error) => {
                result = Err(error);
                break;
            }
        };
        if let Err(error) = heap.set_element(array, index, Value::object(object)) {
            result = Err(error);
            break;
        }
    }
    assert_eq!(result, Err(HeapError::OutOfMemory));
    assert!(heap.bytes() <= limit, "{} > {limit}", heap.bytes());
    // Garbage under the cap is reclaimed instead of failing.
    heap.set_array_length(array, 0);
    for _ in 0..10_000 {
        heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    }
    assert!(heap.bytes() <= limit);
}

#[test]
fn trigger_collects_garbage_without_explicit_calls() {
    let mut heap = Heap::default();
    heap.gc_mut().min_heap = 64 << 10;
    for _ in 0..100_000 {
        heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    }
    assert!(heap.gc().stats.collections > 0);
    assert!(heap.live_objects() < 100_000);
}

/// The stress mode turns a missed root into a deterministic panic at the
/// access that used the stale index.
#[test]
#[cfg(feature = "gc-stress")]
#[should_panic(expected = "stale")]
fn stress_mode_catches_a_missed_root() {
    let mut heap = Heap::default();
    let x = key(&mut heap, "x");
    let unrooted = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    // This allocation collects first; `unrooted` is freed and poisoned.
    let _other = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    let _ = heap.lookup_own(unrooted, x);
}
