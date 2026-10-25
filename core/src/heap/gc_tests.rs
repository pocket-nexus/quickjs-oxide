use super::super::object::InternalData;
use super::super::table::Table;
use super::*;

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

    // Only the Symbol remains rooted; its unflattened description owns both halves.
    heap.roots().truncate(RootMark(0));
    heap.roots().push(Value::symbol(symbol));
    assert!(heap.strings.get(rope.index()).flat().is_none());
    heap.collect();

    assert_eq!(
        (
            heap.is_live(Value::string(left)),
            heap.is_live(Value::string(right)),
        ),
        (true, true),
        "a live Symbol's rope description must retain both children"
    );
    assert_eq!(
        heap.string_text(heap.symbol_description(symbol).unwrap()),
        "leftright"
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
