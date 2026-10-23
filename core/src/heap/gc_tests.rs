use super::super::object::InternalData;
use super::super::table::Table;
use super::*;

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
