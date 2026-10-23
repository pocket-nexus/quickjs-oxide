//! Exhaustive finite-graph tests of the complete collector, not SMT proofs.
use super::super::object::InternalData;
use super::super::table::Table;
use super::*;

#[test]
fn all_two_object_graphs_match_independent_reachability() {
    for edges in 0u8..16 {
        for roots in 0u8..4 {
            let mut heap = Heap::default();
            heap.set_gc_manual(true);
            let a = heap.alloc_scope(None, 2).unwrap();
            heap.roots().push(Value::object(a.object()));
            let b = heap.alloc_scope(None, 2).unwrap();
            heap.roots().push(Value::object(b.object()));
            let objects = [a, b];
            for (i, object) in objects.iter().enumerate() {
                for (j, target) in objects.iter().enumerate() {
                    if edges & (1 << (2 * i + j)) != 0 {
                        heap.scope_set(*object, 0, j as u32, Value::object(target.object()));
                    }
                }
            }
            heap.roots.stack.truncate(RootMark(0));
            for (i, object) in objects.iter().enumerate() {
                if roots & (1 << i) != 0 {
                    heap.roots().push(Value::object(object.object()));
                }
            }
            let mut reachable = [roots & 1 != 0, roots & 2 != 0];
            for _ in 0..2 {
                let before = reachable;
                for (i, source) in before.iter().enumerate() {
                    for (j, target) in reachable.iter_mut().enumerate() {
                        *target |= *source && edges & (1 << (2 * i + j)) != 0;
                    }
                }
            }
            heap.collect();
            for (i, object) in objects.iter().enumerate() {
                assert_eq!(
                    heap.is_live(Value::object(object.object())),
                    reachable[i],
                    "edges={edges:04b}, roots={roots:02b}, object={i}"
                );
            }
            heap.roots.stack.truncate(RootMark(0));
            heap.collect();
            for object in objects {
                assert!(!heap.is_live(Value::object(object.object())));
            }
        }
    }
}

#[test]
fn ephemerons_require_both_live_map_and_live_key() {
    for root_map in [false, true] {
        for root_key in [false, true] {
            let mut heap = Heap::default();
            heap.set_gc_manual(true);
            let key = heap.alloc_scope(None, 0).unwrap().object();
            heap.roots().push(Value::object(key));
            let value = heap.alloc_scope(None, 0).unwrap().object();
            heap.roots().push(Value::object(value));
            let map = heap
                .alloc_internal(
                    ObjectKind::Builtin,
                    InternalTag::WeakMap,
                    None,
                    0,
                    &[],
                    InternalData::Table(Box::new(Table::new(2))),
                    0,
                )
                .unwrap();
            heap.roots().push(Value::object(map));
            heap.table_set(map, Value::object(key), Value::object(value))
                .unwrap();
            heap.roots.stack.truncate(RootMark(0));
            if root_map {
                heap.roots().push(Value::object(map));
            }
            if root_key {
                heap.roots().push(Value::object(key));
            }
            heap.collect();
            assert_eq!(heap.is_live(Value::object(map)), root_map);
            assert_eq!(heap.is_live(Value::object(key)), root_key);
            assert_eq!(heap.is_live(Value::object(value)), root_map && root_key);
            if root_map {
                assert_eq!(heap.table(map).len(), usize::from(root_key));
            }
        }
    }
}

#[test]
fn handle_root_survives_until_release() {
    let mut heap = Heap::default();
    heap.set_gc_manual(true);
    let value = Value::object(heap.alloc_scope(None, 0).unwrap().object());
    let handle = heap.roots.handles.create(value);
    heap.collect();
    assert!(heap.is_live(value));
    assert_eq!(heap.roots.handles.get(handle), value);
    heap.roots.handles.release(handle);
    heap.collect();
    assert!(!heap.is_live(value));
}

#[test]
fn ephemeron_chains_reach_a_fixpoint_in_either_root_order() {
    for roots in 0u8..16 {
        for reverse in [false, true] {
            let mut heap = Heap::default();
            heap.set_gc_manual(true);
            let mut objects = Vec::new();
            for _ in 0..3 {
                let object = heap.alloc_scope(None, 0).unwrap().object();
                heap.roots().push(Value::object(object));
                objects.push(object);
            }
            let mut maps = Vec::new();
            for i in 0..2 {
                let map = heap
                    .alloc_internal(
                        ObjectKind::Builtin,
                        InternalTag::WeakMap,
                        None,
                        0,
                        &[],
                        InternalData::Table(Box::new(Table::new(2))),
                        0,
                    )
                    .unwrap();
                heap.roots().push(Value::object(map));
                heap.table_set(
                    map,
                    Value::object(objects[i]),
                    Value::object(objects[i + 1]),
                )
                .unwrap();
                maps.push(map);
            }
            heap.roots.stack.truncate(RootMark(0));
            for at in 0..2 {
                let i = if reverse { 1 - at } else { at };
                if roots & (1 << i) != 0 {
                    heap.roots().push(Value::object(maps[i]));
                }
            }
            for (i, object) in objects.iter().take(2).enumerate() {
                if roots & (4 << i) != 0 {
                    heap.roots().push(Value::object(*object));
                }
            }
            // map0: key0 -> key1; map1: key1 -> value.
            let key0 = roots & 4 != 0;
            let key1 = roots & 8 != 0 || (roots & 1 != 0 && key0);
            let value = roots & 2 != 0 && key1;
            heap.collect();
            for (i, expected) in [key0, key1, value].into_iter().enumerate() {
                assert_eq!(
                    heap.is_live(Value::object(objects[i])),
                    expected,
                    "roots={roots:04b}, reverse={reverse}, object={i}"
                );
            }
            for (i, map) in maps.iter().enumerate() {
                assert_eq!(heap.is_live(Value::object(*map)), roots & (1 << i) != 0);
            }
        }
    }
}

#[test]
fn verification_does_not_root_a_dead_weak_map_in_the_next_collection() {
    let mut heap = Heap::default();
    heap.set_gc_manual(true);
    let key = heap.alloc_scope(None, 0).unwrap().object();
    heap.roots().push(Value::object(key));
    let value = heap.alloc_scope(None, 0).unwrap().object();
    heap.roots().push(Value::object(value));
    let map = heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::WeakMap,
            None,
            0,
            &[],
            InternalData::Table(Box::new(Table::new(2))),
            0,
        )
        .unwrap();
    heap.roots().push(Value::object(map));
    heap.table_set(map, Value::object(key), Value::object(value))
        .unwrap();
    heap.collect();
    heap.verify();
    heap.verify(); // Repeated diagnostics must not accumulate semantic roots.
    heap.roots.stack.truncate(RootMark(0));
    heap.roots().push(Value::object(key));
    heap.collect();
    assert!(heap.is_live(Value::object(key)));
    assert!(!heap.is_live(Value::object(map)));
    assert!(!heap.is_live(Value::object(value)));
}
