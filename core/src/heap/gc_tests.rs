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

#[test]
fn symbol_keeps_a_nested_rope_description_and_every_leaf() {
    let mut heap = Heap::default();
    let mut leaves = Vec::new();
    for text in ["a", "b", "c"] {
        let leaf = heap.alloc_string(text).unwrap();
        heap.roots().push(Value::string(leaf));
        leaves.push(leaf);
    }
    let inner = heap.alloc_rope(leaves[0], leaves[1]).unwrap();
    heap.roots().push(Value::string(inner));
    let outer = heap.alloc_rope(inner, leaves[2]).unwrap();
    heap.roots().push(Value::string(outer));
    let symbol = heap.alloc_symbol(Some(outer), false).unwrap();

    // Only the Symbol remains rooted; the leaves are two ropes below it.
    heap.roots().truncate(RootMark(0));
    heap.roots().push(Value::symbol(symbol));
    assert!(heap.strings.get(outer.index()).flat().is_none());
    assert!(heap.strings.get(inner.index()).flat().is_none());
    heap.collect();
    heap.verify();

    assert!(
        leaves.iter().all(|&leaf| heap.is_live(Value::string(leaf))),
        "a live Symbol's nested rope description must retain every leaf"
    );
    assert_eq!(
        heap.string_text(heap.symbol_description(symbol).unwrap()),
        "abc"
    );
}

#[test]
#[should_panic(expected = "(left half of rope")]
fn edge_check_reports_a_freed_rope_half() {
    let mut heap = Heap::default();
    let left = heap.alloc_string("left").unwrap();
    heap.roots().push(Value::string(left));
    let right = heap.alloc_string("right").unwrap();
    heap.roots().push(Value::string(right));
    let rope = heap.alloc_rope(left, right).unwrap();
    heap.roots().push(Value::string(rope));
    heap.collect();
    heap.verify_edges();

    heap.strings.free(left.index());
    heap.verify_edges();
}

#[test]
#[should_panic(expected = "(description of symbol")]
fn edge_check_reports_a_freed_symbol_description() {
    let mut heap = Heap::default();
    let description = heap.alloc_string("description").unwrap();
    heap.roots().push(Value::string(description));
    let symbol = heap.alloc_symbol(Some(description), false).unwrap();
    heap.roots().push(Value::symbol(symbol));
    heap.collect();
    heap.verify_edges();

    heap.strings.free(description.index());
    heap.verify_edges();
}

#[test]
#[should_panic(expected = "gc verify: frame 0 runs bytecode")]
fn edge_check_reports_a_frame_running_another_functions_bytecode() {
    let mut heap = Heap::default();
    let mut functions = Vec::new();
    for unit in 0..2 {
        let tree = [super::super::BytecodeTemplate {
            unit: crate::bytecode::CodeUnitId(unit),
            constants: &[],
            children: &[],
        }];
        let function = heap.alloc_bytecode_function(None, None, &tree).unwrap();
        heap.roots().push(Value::object(function));
        functions.push(function);
    }
    let bytecode =
        |heap: &Heap, function| crate::value::BytecodeId(heap.function_record(function).bytecode);
    let frame = |function, bytecode, unit| crate::vm::Frame {
        unit: crate::bytecode::CodeUnitId(unit),
        function,
        bytecode,
        base: 0,
        return_pc: 0,
        return_dst: 0,
        argc: 0,
        flags: 0,
    };
    let own = bytecode(&heap, functions[0]);
    heap.vm.frames.push(frame(functions[0], own, 0));
    heap.collect();
    heap.verify_edges();

    let other = bytecode(&heap, functions[1]);
    heap.vm.frames[0] = frame(functions[0], other, 1);
    heap.verify_edges();
}
