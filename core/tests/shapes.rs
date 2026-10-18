//! Shapes: the transition tree, dictionary mode, dense arrays, scopes and
//! validity cells.

use oxide_core::heap::gc::HeapLimits;
use oxide_core::shape::{DICTIONARY_THRESHOLD, TABLE_THRESHOLD};
use oxide_core::{
    Heap, HeapError, Intrinsic, ObjectId, ObjectKind, PropertyFlags, PropertyKey, Runtime, Value,
};

const DEFAULT: PropertyFlags = PropertyFlags::DEFAULT;

fn held(heap: &mut Heap, prototype: Option<ObjectId>) -> ObjectId {
    let object = heap
        .alloc_object(ObjectKind::Ordinary, prototype, 4)
        .unwrap();
    heap.handles().create(Value::object(object));
    object
}

fn index(value: u32) -> PropertyKey {
    PropertyKey::index(value).unwrap()
}

fn add(heap: &mut Heap, object: ObjectId, key: PropertyKey) {
    heap.add_property(object, key, DEFAULT, Value::int32(key.raw() as i32))
        .unwrap();
}

#[test]
fn lookup_table_exists_only_above_eight_properties() {
    let mut heap = Heap::default();
    let object = held(&mut heap, None);
    let keys: Vec<_> = (1..=9).map(index).collect();
    for &key in &keys[..8] {
        add(&mut heap, object, key);
    }
    for (slot, &key) in keys[..8].iter().enumerate() {
        assert_eq!(heap.lookup_own(object, key), Some((slot as u32, DEFAULT)));
    }
    assert_eq!(TABLE_THRESHOLD, 8);
    assert!(!heap.shape(heap.shape_of(object)).has_table());
    add(&mut heap, object, keys[8]);
    assert_eq!(heap.lookup_own(object, keys[8]), Some((8, DEFAULT)));
    assert!(heap.shape(heap.shape_of(object)).has_table());
    for (slot, &key) in keys.iter().enumerate() {
        assert_eq!(heap.lookup_own(object, key), Some((slot as u32, DEFAULT)));
        assert_eq!(
            heap.get_slot(object, slot as u32),
            Value::int32(key.raw() as i32)
        );
    }
}

#[test]
fn lookup_positions_and_duplicate_keys() {
    let mut heap = Heap::default();
    let object = held(&mut heap, None);
    let (first, second) = (index(1), index(2));
    add(&mut heap, object, first);
    add(&mut heap, object, second);
    assert_eq!(heap.lookup_own(object, first), Some((0, DEFAULT)));
    assert_eq!(heap.lookup_own(object, second), Some((1, DEFAULT)));
    assert_eq!(heap.lookup_own(object, index(3)), None);
    assert_eq!(heap.prototype_of(object), None);
    assert_eq!(
        heap.add_property(object, first, DEFAULT, Value::NULL),
        Err(HeapError::DuplicateKey)
    );
    assert_eq!(heap.get_slot(object, 0), Value::int32(first.raw() as i32));
}

#[test]
fn reflag_keeps_slot_delete_and_readd_moves_to_end() {
    let mut heap = Heap::default();
    let alpha = heap.property_key("alpha").unwrap();
    let beta = heap.property_key("beta").unwrap();
    let object = held(&mut heap, None);
    let twin = held(&mut heap, None);
    for target in [object, twin] {
        add(&mut heap, target, alpha);
        add(&mut heap, target, beta);
    }
    assert_eq!(
        heap.shape_of(object),
        heap.shape_of(twin),
        "shapes are shared"
    );

    // A reflag keeps the slot's width; data and accessor properties take
    // one and two slots, so a change between them is a delete and an add.
    let read_only = PropertyFlags::data(false, false, true);
    assert!(heap.set_property_flags(object, alpha, read_only).unwrap());
    assert_eq!(heap.lookup_own(object, alpha), Some((0, read_only)));
    assert_eq!(
        heap.lookup_own(twin, alpha),
        Some((0, DEFAULT)),
        "the shared shape is unchanged"
    );
    assert!(!heap.is_dictionary(object));

    assert!(heap.delete_property(object, alpha).unwrap());
    assert!(heap.is_dictionary(object));
    assert_eq!(heap.lookup_own(object, alpha), None);
    assert_eq!(heap.lookup_own(object, beta), Some((1, DEFAULT)));
    heap.add_property(object, alpha, DEFAULT, Value::TRUE)
        .unwrap();
    assert_eq!(heap.own_keys(object), [beta, alpha]);
    let (slot, _) = heap.lookup_own(object, alpha).unwrap();
    assert_eq!(heap.get_slot(object, slot), Value::TRUE);
    assert_eq!(heap.get_slot(object, 1), Value::int32(beta.raw() as i32));

    assert_eq!(
        heap.add_property(object, alpha, DEFAULT, Value::NULL),
        Err(HeapError::DuplicateKey)
    );
    let missing = heap.property_key("missing").unwrap();
    assert!(!heap.set_property_flags(object, missing, DEFAULT).unwrap());
    assert!(!heap.delete_property(object, missing).unwrap());
    assert_eq!(heap.own_keys(twin), [alpha, beta]);
}

#[test]
fn own_keys_order_indices_strings_symbols_and_hide_private_names() {
    let mut heap = Heap::default();
    let object = held(&mut heap, None);
    let beta = heap.property_key("beta").unwrap();
    let index_10 = heap.property_key("10").unwrap();
    let a = heap.intern("a").unwrap();
    let symbol_a = PropertyKey::symbol(heap.alloc_symbol(Some(a), false).unwrap());
    heap.roots().push(symbol_a.to_value()); // symbols are not atoms: root them
    let index_2 = heap.property_key("2").unwrap();
    let noncanonical = heap.property_key("01").unwrap();
    let hidden = heap.intern("hidden").unwrap();
    let private = PropertyKey::symbol(heap.alloc_symbol(Some(hidden), true).unwrap());
    heap.roots().push(private.to_value()); // symbols are not atoms: root them
    let shared = heap.intern("shared").unwrap();
    let global_symbol = PropertyKey::symbol(heap.alloc_symbol(Some(shared), false).unwrap());
    heap.roots().push(global_symbol.to_value()); // symbols are not atoms: root them
    let largest = heap.property_key("4294967294").unwrap();
    let excluded = heap.property_key("4294967295").unwrap();
    for key in [
        beta,
        index_10,
        symbol_a,
        index_2,
        noncanonical,
        private,
        global_symbol,
        largest,
        excluded,
    ] {
        add(&mut heap, object, key);
    }
    assert_eq!(
        heap.own_keys(object),
        [
            index_2,
            index_10,
            largest,
            beta,
            noncanonical,
            excluded,
            symbol_a,
            global_symbol
        ]
    );
}

#[test]
fn delete_then_readd_leaves_other_objects_alone() {
    let mut heap = Heap::default();
    let first = heap.property_key("first").unwrap();
    let second = heap.property_key("second").unwrap();
    let changed = held(&mut heap, None);
    let unchanged = held(&mut heap, None);
    for target in [changed, unchanged] {
        add(&mut heap, target, first);
        add(&mut heap, target, second);
    }
    heap.delete_property(changed, first).unwrap();
    add(&mut heap, changed, first);
    assert_eq!(heap.own_keys(changed), [second, first]);
    assert_eq!(heap.own_keys(unchanged), [first, second]);
}

/// Replaces `own_key_snapshot_validates_the_runtime_local_atom_table`: keys
/// are indices into this heap, so a key naming a freed atom is caught by the
/// liveness assertion instead of a table comparison.
#[test]
#[cfg(any(debug_assertions, feature = "gc-stress"))]
#[should_panic(expected = "stale")]
fn key_naming_a_dead_string_is_caught() {
    let mut heap = Heap::default();
    let object = held(&mut heap, None);
    let text = heap.alloc_string("not-an-atom").unwrap();
    let mark = heap.roots().mark();
    heap.roots().push(Value::string(text));
    heap.roots().truncate(mark);
    heap.collect();
    // Building a key from a freed string trips the assertion when used.
    let key = PropertyKey::atom(text);
    add(&mut heap, object, key);
    let _ = heap.own_keys(object);
}

#[test]
fn many_properties_enter_dictionary_mode() {
    let mut heap = Heap::default();
    let object = held(&mut heap, None);
    for value in 0..=DICTIONARY_THRESHOLD {
        add(&mut heap, object, index(value));
    }
    assert!(heap.is_dictionary(object));
    for value in 0..=DICTIONARY_THRESHOLD {
        let (slot, _) = heap.lookup_own(object, index(value)).unwrap();
        assert_eq!(slot, value);
        assert_eq!(
            heap.get_slot(object, slot),
            Value::int32(index(value).raw() as i32)
        );
    }
    // Deleted slots are reused; order stays insertion order.
    for value in (0..DICTIONARY_THRESHOLD).step_by(2) {
        assert!(heap.delete_property(object, index(value)).unwrap());
    }
    let readd = index(1000);
    add(&mut heap, object, readd);
    assert_eq!(*heap.own_keys(object).last().unwrap(), readd);
    heap.collect();
    assert_eq!(
        heap.own_keys(object).len() as u32,
        DICTIONARY_THRESHOLD / 2 + 2
    );
}

#[test]
fn global_object_is_a_dictionary() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let global = runtime.global_object();
    let name = runtime.heap_mut().property_key("selfReference").unwrap();
    assert!(runtime.heap().is_dictionary(global));
    runtime
        .heap_mut()
        .add_property(global, name, DEFAULT, Value::object(global))
        .unwrap();
    runtime.heap_mut().collect();
    assert_eq!(
        runtime.heap().get_property(global, name),
        Some(Value::object(global))
    );
    let object_prototype = runtime.intrinsic(Intrinsic::ObjectPrototype);
    assert_eq!(runtime.heap().prototype_of(global), Some(object_prototype));
}

#[test]
fn dense_arrays_with_holes() {
    let mut runtime = Runtime::new(HeapLimits::default()).unwrap();
    let array = runtime.new_array(2).unwrap();
    let handle = runtime.hold(Value::object(array));
    let heap = runtime.heap_mut();
    heap.push_element(array, Value::int32(0)).unwrap();
    heap.push_element(array, Value::int32(1)).unwrap();
    heap.set_element(array, 5, Value::int32(5)).unwrap();
    assert_eq!(heap.array_length(array), 6);
    assert_eq!(heap.get_element(array, 1), Value::int32(1));
    assert!(heap.get_element(array, 3).is_empty(), "hole");
    assert!(heap.get_element(array, 6).is_empty(), "past the end");
    let keys: Vec<_> = heap.own_keys(array);
    assert_eq!(keys, [index(0), index(1), index(5)]);
    heap.set_array_length(array, 1);
    assert!(heap.get_element(array, 1).is_empty());
    heap.set_array_length(array, 6);
    assert!(
        heap.get_element(array, 5).is_empty(),
        "truncation cleared the element"
    );
    runtime.release(handle);
    runtime.heap_mut().collect();
    assert!(!runtime.heap().is_live(Value::object(array)));
}

#[test]
fn prototype_chain_reads_and_set_prototype() {
    let mut heap = Heap::default();
    let x = heap.property_key("x").unwrap();
    let grand = held(&mut heap, None);
    heap.add_property(grand, x, DEFAULT, Value::int32(3))
        .unwrap();
    let parent = held(&mut heap, Some(grand));
    let child = held(&mut heap, Some(parent));
    let object = held(&mut heap, Some(child));
    assert_eq!(heap.get_property(object, x), Some(Value::int32(3)));
    assert_eq!(
        heap.set_prototype(grand, Some(object)),
        Err(HeapError::PrototypeCycle)
    );
    heap.set_prototype(object, None).unwrap();
    assert_eq!(heap.get_property(object, x), None);
}

#[test]
fn validity_cells_invalidate_transitively() {
    let mut heap = Heap::default();
    let x = heap.property_key("x").unwrap();
    let y = heap.property_key("y").unwrap();
    // a <- b <- c <- receiver, and an unrelated chain other <- side.
    let a = held(&mut heap, None);
    let b = held(&mut heap, Some(a));
    let c = held(&mut heap, Some(b));
    let receiver = held(&mut heap, Some(c));
    let other = held(&mut heap, None);
    let side = held(&mut heap, Some(other));

    let cell = heap.validity_cell(receiver).unwrap();
    let cell_c = heap.validity_cell(c).unwrap();
    let side_cell = heap.validity_cell(side).unwrap();
    assert!(heap.cell_is_valid(cell) && heap.cell_is_valid(cell_c));
    assert_eq!(
        heap.validity_cell(receiver).unwrap(),
        cell,
        "stable while nothing changes"
    );

    // Changing the receiver itself does not invalidate.
    heap.add_property(receiver, y, DEFAULT, Value::NULL)
        .unwrap();
    assert!(heap.cell_is_valid(cell));

    heap.add_property(a, x, DEFAULT, Value::int32(1)).unwrap();
    assert!(
        !heap.cell_is_valid(cell),
        "a change at the top reaches the receiver's cell"
    );
    assert!(!heap.cell_is_valid(cell_c));
    assert!(
        heap.cell_is_valid(side_cell),
        "unrelated chains keep their cells"
    );

    let fresh = heap.validity_cell(receiver).unwrap();
    assert_ne!(fresh, cell);
    assert!(heap.cell_is_valid(fresh));

    // Deletion, reflagging and re-parenting a prototype all invalidate.
    heap.delete_property(b, x).unwrap(); // absent: no change
    assert!(heap.cell_is_valid(fresh));
    heap.set_property_flags(a, x, PropertyFlags::data(false, true, true))
        .unwrap();
    assert!(!heap.cell_is_valid(fresh));
    let fresh = heap.validity_cell(receiver).unwrap();
    heap.delete_property(a, x).unwrap();
    assert!(!heap.cell_is_valid(fresh));
    let fresh = heap.validity_cell(receiver).unwrap();
    heap.set_prototype(b, Some(other)).unwrap();
    assert!(!heap.cell_is_valid(fresh));
    let fresh = heap.validity_cell(receiver).unwrap();
    // b now inherits from `other`, so a change to `other` reaches the receiver
    // and `a` no longer does.
    heap.add_property(a, y, DEFAULT, Value::NULL).unwrap();
    assert!(heap.cell_is_valid(fresh));
    heap.add_property(other, y, DEFAULT, Value::NULL).unwrap();
    assert!(!heap.cell_is_valid(fresh));
    assert!(!heap.cell_is_valid(side_cell));

    heap.collect();
    let root = heap.validity_cell(a).unwrap();
    assert!(
        heap.cell_is_valid(root),
        "an object without a prototype uses the static cell"
    );
}

#[test]
fn cells_and_prototype_infos_die_with_their_objects() {
    let mut heap = Heap::default();
    let prototype = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    let handle = heap.handles().create(Value::object(prototype));
    let object = held(&mut heap, Some(prototype));
    heap.validity_cell(object).unwrap();
    assert_eq!(heap.live_cells(), 2);
    heap.handles().release(handle);
    heap.collect();
    assert_eq!(heap.live_cells(), 2, "the object keeps its prototype alive");
    heap.set_prototype(object, None).unwrap();
    heap.collect();
    assert!(!heap.is_live(Value::object(prototype)));
    assert_eq!(heap.live_cells(), 1, "only the static cell is left");
}

/// An accessor takes two consecutive slots (getter, setter) in the transition
/// tree and in dictionary mode, and the properties after it keep their own.
#[test]
fn accessors_take_two_slots() {
    let mut heap = Heap::default();
    let a = heap.property_key("a").unwrap();
    let get = heap.property_key("get").unwrap();
    let b = heap.property_key("b").unwrap();
    let c = heap.property_key("c").unwrap();
    let accessor = PropertyFlags::accessor(true, true);
    let object = held(&mut heap, None);
    heap.add_property(object, a, DEFAULT, Value::int32(1))
        .unwrap();
    heap.add_accessor(object, get, accessor, Value::int32(10), Value::int32(11))
        .unwrap();
    heap.add_property(object, b, DEFAULT, Value::int32(2))
        .unwrap();
    assert_eq!(heap.lookup_own(object, get), Some((1, accessor)));
    assert_eq!(heap.lookup_own(object, b), Some((3, DEFAULT)));
    assert_eq!(heap.get_slot(object, 1), Value::int32(10));
    assert_eq!(heap.get_slot(object, 2), Value::int32(11));
    assert_eq!(heap.get_slot(object, 3), Value::int32(2));
    let shape = heap.shape_of(object);
    assert_eq!((heap.shape(shape).count, heap.shape(shape).width), (3, 4));
    // A twin reaches the same shape through the shared transitions.
    let twin = held(&mut heap, None);
    heap.add_property(twin, a, DEFAULT, Value::int32(1))
        .unwrap();
    heap.add_accessor(twin, get, accessor, Value::UNDEFINED, Value::UNDEFINED)
        .unwrap();
    heap.add_property(twin, b, DEFAULT, Value::int32(2))
        .unwrap();
    assert_eq!(heap.shape_of(twin), shape);

    // Dictionary mode keeps the layout; a delete frees both slots and a
    // later accessor takes two fresh ones.
    assert!(heap.delete_property(object, a).unwrap());
    assert!(heap.is_dictionary(object));
    assert_eq!(heap.lookup_own(object, get), Some((1, accessor)));
    assert_eq!(heap.lookup_own(object, b), Some((3, DEFAULT)));
    assert!(heap.delete_property(object, get).unwrap());
    heap.add_accessor(object, get, accessor, Value::int32(20), Value::int32(21))
        .unwrap();
    assert_eq!(heap.lookup_own(object, get), Some((4, accessor)));
    assert_eq!(heap.get_slot(object, 4), Value::int32(20));
    assert_eq!(heap.get_slot(object, 5), Value::int32(21));
    heap.add_property(object, c, DEFAULT, Value::int32(3))
        .unwrap();
    let (slot, _) = heap.lookup_own(object, c).unwrap();
    assert!(slot < 3, "a data property reuses a freed slot");
    assert_eq!(heap.get_slot(object, 3), Value::int32(2));
    for _ in 0..2 {
        heap.collect();
    }
    assert_eq!(heap.get_slot(object, 5), Value::int32(21));
}
