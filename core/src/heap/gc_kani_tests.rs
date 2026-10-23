use super::*;

#[kani::proof]
#[kani::unwind(8)]
fn root_stack_scopes() {
    let mut roots = RootStack::default();
    let outer = roots.mark();
    let a = Value::int32(kani::any());
    let b = Value::int32(kani::any());
    let c = Value::int32(kani::any());
    roots.push(a);
    let inner = roots.mark();
    roots.push(b);
    roots.set(1, c);
    assert_eq!(roots.get(0), a);
    assert_eq!(roots.get(1), c);
    roots.truncate(inner);
    assert_eq!(roots.len(), 1);
    assert_eq!(roots.get(0), a);
    roots.push(b);
    roots.pop_one();
    assert_eq!(roots.values(), &[a]);
    roots.truncate(outer);
    assert!(roots.is_empty());
}

#[kani::proof]
#[kani::unwind(8)]
fn handles_preserve_other_roots_and_reuse() {
    let mut handles = Handles::default();
    let a = Value::int32(kani::any());
    let b = Value::int32(kani::any());
    let c = Value::int32(kani::any());
    let first = handles.create(a);
    let second = handles.create(b);
    handles.set(first, c);
    assert_eq!(handles.get(first), c);
    assert_eq!(handles.get(second), b);
    handles.release(first);
    assert_eq!(handles.live_count(), 1);
    let third = handles.create(a);
    assert_eq!(third, first);
    assert_eq!(handles.get(second), b);
    assert_eq!(handles.get(third), a);
    assert_eq!(handles.live_count(), 2);
    handles.release(second);
    handles.release(third);
    assert_eq!(handles.values().count(), 0);
}

#[kani::proof]
#[kani::unwind(24)]
fn strong_graph_marking() {
    let mut heap = Heap::default();
    heap.set_gc_manual(true);
    let a = heap.alloc_scope(None, 2).unwrap();
    heap.roots().push(Value::object(a.object()));
    let b = heap.alloc_scope(None, 2).unwrap();
    heap.roots().push(Value::object(b.object()));
    let objects = [a, b];
    let edges: u8 = kani::any();
    kani::assume(edges < 16);
    let roots: u8 = kani::any();
    kani::assume(roots < 4);
    for i in 0..2 {
        for j in 0..2 {
            if edges & (1 << (2 * i + j)) != 0 {
                heap.scope_set(objects[i], 0, j as u32, Value::object(objects[j].object()));
            }
        }
    }
    heap.roots.stack.truncate(RootMark(0));
    for i in 0..2 {
        if roots & (1 << i) != 0 {
            heap.roots().push(Value::object(objects[i].object()));
        }
    }
    // Independent Boolean transitive closure, including cycles and self edges.
    let mut reachable = [roots & 1 != 0, roots & 2 != 0];
    for _ in 0..2 {
        let before = reachable;
        for i in 0..2 {
            for j in 0..2 {
                reachable[j] |= before[i] && edges & (1 << (2 * i + j)) != 0;
            }
        }
    }
    // Actual marking kernel, checked before reclamation. Sweep integration,
    // timing and budget bookkeeping are separate obligations; no stubs.
    heap.mark_from_roots();
    for i in 0..2 {
        assert_eq!(
            heap.value_marked(Value::object(objects[i].object())),
            reachable[i]
        );
    }
}

#[kani::proof]
#[kani::unwind(24)]
fn ephemeron_requires_reachable_map_and_key() {
    use super::super::object::InternalData;
    use super::super::table::Table;

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
    let root_map: bool = kani::any();
    let root_key: bool = kani::any();
    heap.roots.stack.truncate(RootMark(0));
    if root_map {
        heap.roots().push(Value::object(map));
    }
    if root_key {
        heap.roots().push(Value::object(key));
    }
    heap.mark_from_roots();
    heap.process_weak();
    // The value is reachable only through the ephemeron, never from a root.
    assert_eq!(heap.value_marked(Value::object(map)), root_map);
    assert_eq!(heap.value_marked(Value::object(key)), root_key);
    assert_eq!(
        heap.value_marked(Value::object(value)),
        root_map && root_key
    );
    if root_map {
        assert_eq!(heap.table(map).len(), usize::from(root_key));
    }
}
