use super::super::object::InternalData;
use super::super::table::Table;
use super::*;

fn weak_map(heap: &mut Heap) -> crate::ObjectId {
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
    map
}

fn held_scope(heap: &mut Heap, slots: u32) -> crate::ScopeId {
    let scope = heap.alloc_scope(None, slots).unwrap();
    heap.roots().push(Value::object(scope.object()));
    scope
}

#[test]
fn symbol_keeps_an_unflattened_rope_description_and_its_halves() {
    let mut heap = Heap::default();
    let left = heap.alloc_string("left").unwrap();
    heap.roots().push(Value::string(left));
    let right = heap.alloc_string("right").unwrap();
    heap.roots().push(Value::string(right));
    let rope = heap.alloc_rope(left, right).unwrap();
    heap.roots().push(Value::string(rope));
    let symbol = heap.alloc_symbol(Some(rope), false).unwrap();
    heap.roots().truncate(RootMark(0));
    heap.roots().push(Value::symbol(symbol));
    assert!(heap.strings.get(rope.index()).flat().is_none());
    heap.collect();
    assert!(heap.is_live(Value::string(left)));
    assert!(heap.is_live(Value::string(right)));
    assert_eq!(
        heap.string_text(heap.symbol_description(symbol).unwrap()),
        "leftright"
    );
    heap.roots().truncate(RootMark(0));
    heap.collect();
    assert!(!heap.is_live(Value::symbol(symbol)));
}

#[test]
fn ephemeron_value_to_key_cycle_does_not_bootstrap_liveness() {
    let mut heap = Heap::default();
    let map = weak_map(&mut heap);
    let map_only = heap.roots().mark();
    let key = held_scope(&mut heap, 0);
    let value = held_scope(&mut heap, 1);
    heap.scope_set(value, 0, 0, Value::object(key.object()));
    heap.table_set(
        map,
        Value::object(key.object()),
        Value::object(value.object()),
    )
    .unwrap();
    heap.roots().truncate(map_only);
    heap.collect();
    assert!(heap.is_live(Value::object(map)));
    assert!(!heap.is_live(Value::object(key.object())));
    assert!(!heap.is_live(Value::object(value.object())));
}

#[test]
fn ephemeron_activation_discovers_a_new_map_and_a_symbol_key() {
    let mut heap = Heap::default();
    let first = weak_map(&mut heap);
    let key = held_scope(&mut heap, 0);
    let retained = heap.roots().mark();
    let second = weak_map(&mut heap);
    let symbol = heap.alloc_symbol(None, false).unwrap();
    heap.roots().push(Value::symbol(symbol));
    let bridge = held_scope(&mut heap, 2);
    let result = held_scope(&mut heap, 0);
    heap.scope_set(bridge, 0, 0, Value::object(second));
    heap.scope_set(bridge, 0, 1, Value::symbol(symbol));
    heap.table_set(
        first,
        Value::object(key.object()),
        Value::object(bridge.object()),
    )
    .unwrap();
    heap.table_set(
        second,
        Value::symbol(symbol),
        Value::object(result.object()),
    )
    .unwrap();
    heap.roots().truncate(retained);
    heap.collect();
    for value in [
        Value::object(second),
        Value::symbol(symbol),
        Value::object(result.object()),
    ] {
        assert!(heap.is_live(value));
    }
    heap.roots().truncate(RootMark(0));
    heap.collect();
    assert!(!heap.is_live(Value::object(result.object())));
}

#[test]
fn finalization_jobs_keep_callback_and_held_value_after_target_dies() {
    let mut heap = Heap::default();
    let callback = held_scope(&mut heap, 0);
    let target = held_scope(&mut heap, 0);
    let held = held_scope(&mut heap, 0);
    let registry = heap
        .alloc_internal(
            ObjectKind::Builtin,
            InternalTag::FinalizationRegistry,
            None,
            0,
            &[
                Value::object(callback.object()),
                Value::object(target.object()),
                Value::object(held.object()),
                Value::UNDEFINED,
            ],
            InternalData::None,
            0,
        )
        .unwrap();
    heap.roots().truncate(RootMark(0));
    heap.roots().push(Value::object(registry));
    heap.collect();
    assert!(!heap.is_live(Value::object(target.object())));
    assert_eq!(heap.jobs.len(), 1);
    heap.roots().truncate(RootMark(0));
    heap.collect();
    assert!(!heap.is_live(Value::object(registry)));
    assert!(heap.is_live(Value::object(callback.object())));
    assert!(heap.is_live(Value::object(held.object())));
    heap.jobs.clear();
    heap.collect();
    assert!(!heap.is_live(Value::object(held.object())));
}

#[test]
fn generated_heap_graphs_match_independent_saturation_across_collections() {
    // Fixed seeds and explicit fixture edges; no collector traversal supplies the oracle.
    for seed in 0..96_u64 {
        let mut rng = seed + 1;
        let mut next = || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            rng as usize
        };
        let mut heap = Heap::default();
        let scopes: Vec<_> = (0..6).map(|_| held_scope(&mut heap, 2)).collect();
        let maps: Vec<_> = (0..3).map(|_| weak_map(&mut heap)).collect();
        let values: Vec<_> = scopes
            .iter()
            .map(|s| Value::object(s.object()))
            .chain(maps.iter().copied().map(Value::object))
            .collect();
        let mut edges = Vec::new();
        for (i, scope) in scopes.iter().enumerate() {
            for slot in 0..2 {
                if next() % 3 != 0 {
                    let target = next() % values.len();
                    heap.scope_set(*scope, 0, slot, values[target]);
                    edges.push((i, target));
                }
            }
        }
        let mut weak = Vec::new();
        for (i, map) in maps.iter().enumerate() {
            for key in 0..6 {
                if next() % 2 == 0 {
                    let value = next() % values.len();
                    heap.table_set(*map, values[key], values[value]).unwrap();
                    weak.push((i + 6, key, value));
                }
            }
        }
        let mut roots: Vec<_> = (0..values.len()).map(|_| next() % 3 == 0).collect();
        for round in 0..3 {
            heap.roots().truncate(RootMark(0));
            for (i, &root) in roots.iter().enumerate() {
                if root {
                    heap.roots().push(values[i]);
                }
            }
            let mut live = roots.clone();
            loop {
                let old = live.clone();
                for &(a, b) in &edges {
                    live[b] |= live[a];
                }
                for &(m, k, v) in &weak {
                    live[v] |= live[m] && live[k];
                }
                if live == old {
                    break;
                }
            }
            heap.collect();
            heap.verify();
            for (i, value) in values.iter().enumerate() {
                assert_eq!(
                    heap.is_live(*value),
                    live[i],
                    "seed {seed}, round {round}, node {i}"
                );
            }
            for root in &mut roots {
                *root &= next() % 2 == 0;
            }
        }
    }
}

#[test]
fn independent_graph_detects_a_missing_scope_trace_edge() {
    let mut heap = Heap::default();
    let parent = held_scope(&mut heap, 1);
    let parent_root = heap.roots().mark();
    let child = held_scope(&mut heap, 0);
    heap.scope_set(parent, 0, 0, Value::object(child.object()));
    heap.roots().truncate(parent_root);
    let snapshot = audit::Snapshot::capture(&heap);
    // Inject the result of a lost trace edge, rather than trusting the collector
    // to provide both expected and actual reachability.
    heap.scope_set(parent, 0, 0, Value::UNDEFINED);
    heap.mark_from_roots();
    let detected =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| snapshot.check_marks(&heap)));
    let error = detected.expect_err("independent graph accepted a missing strong edge");
    let message = error
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| error.downcast_ref::<&str>().copied())
        .unwrap_or("");
    assert!(
        message.contains("gc graph: wrong mark"),
        "unexpected panic: {message}"
    );
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

    // Keep only the key rooted: the dead map must not keep its value alive.
    heap.roots.stack.truncate(RootMark(0));
    heap.roots().push(Value::object(key));
    heap.collect();
    assert!(heap.is_live(Value::object(key)));
    assert!(!heap.is_live(Value::object(map)));
    assert!(!heap.is_live(Value::object(value)));
}
