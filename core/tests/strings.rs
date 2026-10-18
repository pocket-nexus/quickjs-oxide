//! Interning, property keys, symbols and BigInt records.

use oxide_core::{Heap, KeyKind, ObjectKind, PropertyFlags, PropertyKey, Value};

#[test]
fn interning_is_by_content() {
    let mut heap = Heap::default();
    let ascii = heap.intern("length").unwrap();
    assert_eq!(heap.intern("length").unwrap(), ascii);
    let units: Vec<u16> = "length".encode_utf16().collect();
    assert_eq!(heap.intern_units(&units).unwrap(), ascii);

    let latin1 = heap.intern("caf\u{e9}").unwrap();
    assert_eq!(
        heap.intern_units(&[0x63, 0x61, 0x66, 0xE9]).unwrap(),
        latin1
    );
    let greek = heap.intern("\u{3b1}\u{3b2}").unwrap();
    assert_eq!(heap.intern_units(&[0x3B1, 0x3B2]).unwrap(), greek);
    let astral = heap.intern("\u{1F600}").unwrap();
    assert_eq!(heap.intern_units(&[0xD83D, 0xDE00]).unwrap(), astral);
    let lone = heap.intern_units(&[0xD800]).unwrap();
    assert_ne!(lone, astral);
    assert_eq!(heap.atom_table().len(), 5);

    let plain = heap.alloc_string("length").unwrap();
    assert_ne!(plain, ascii);
    assert!(!heap.string(plain).is_atom && heap.string(ascii).is_atom);
    assert_eq!(heap.intern_string(plain).unwrap(), ascii);
    assert_eq!(heap.intern_string(ascii).unwrap(), ascii);
    assert_eq!(heap.atom_table().len(), 5);
    assert_eq!(heap.string_text(greek), "\u{3b1}\u{3b2}");
    assert_eq!(
        heap.property_key("length").unwrap(),
        PropertyKey::atom(ascii)
    );
}

#[test]
fn index_keys_stay_out_of_the_atom_table() {
    let mut heap = Heap::default();
    for text in ["0", "1", "42", "2147483647"] {
        let key = heap.property_key(text).unwrap();
        assert!(matches!(key.kind(), KeyKind::Index(_)), "{text}");
    }
    assert_eq!(
        heap.property_key("7").unwrap(),
        PropertyKey::index(7).unwrap()
    );
    let units: Vec<u16> = "99".encode_utf16().collect();
    assert_eq!(
        heap.property_key_units(&units).unwrap(),
        PropertyKey::index(99).unwrap()
    );
    assert!(heap.atom_table().is_empty());

    // Not canonical, or too large for an immediate: atoms.
    for text in ["01", "-1", "1.5", "4294967295", "2147483648"] {
        let key = heap.property_key(text).unwrap();
        assert!(matches!(key.kind(), KeyKind::String(_)), "{text}");
    }
    assert_eq!(heap.atom_table().len(), 5);
    let KeyKind::String(large) = heap.property_key("2147483648").unwrap().kind() else {
        unreachable!()
    };
    assert_eq!(
        heap.string(large).array_index,
        1 << 31,
        "large indices still order as indices"
    );
}

#[test]
fn atoms_survive_collection() {
    let mut heap = Heap::default();
    let atom = heap.intern("survivor").unwrap();
    let plain = heap.alloc_string("transient").unwrap();
    heap.collect();
    assert!(heap.is_live(Value::string(atom)));
    assert!(!heap.is_live(Value::string(plain)));
    assert_eq!(heap.intern("survivor").unwrap(), atom);
    assert_eq!(heap.string_text(atom), "survivor");
}

#[test]
fn symbols_and_bigints_are_traced() {
    let mut heap = Heap::default();
    let holder = heap.alloc_object(ObjectKind::Ordinary, None, 4).unwrap();
    let handle = heap.handles().create(Value::object(holder));
    let description = heap.alloc_string("tag").unwrap();
    heap.set_element(holder, 0, Value::string(description))
        .unwrap();
    let symbol = heap.alloc_symbol(Some(description), false).unwrap();
    heap.set_element(holder, 0, Value::symbol(symbol)).unwrap();
    let big = heap.alloc_bigint(true, &[1, 2, 0, 0]).unwrap();
    heap.set_element(holder, 1, Value::bigint(big)).unwrap();
    let keyed = heap.alloc_symbol(None, false).unwrap();
    let key = PropertyKey::symbol(keyed);
    heap.add_property(holder, key, PropertyFlags::DEFAULT, Value::TRUE)
        .unwrap();
    let orphan = heap.alloc_bigint(false, &[7]).unwrap();
    heap.collect();

    assert!(heap.is_live(Value::symbol(symbol)));
    assert_eq!(
        heap.symbol_description(symbol),
        Some(description),
        "the description is traced"
    );
    assert!(heap.is_live(Value::string(description)));
    assert_eq!(
        &*heap.bigint(big).limbs,
        &[1, 2],
        "trailing zero limbs are trimmed"
    );
    assert!(heap.bigint(big).negative);
    assert!(
        heap.is_live(Value::symbol(keyed)),
        "a key in a shape is traced"
    );
    assert!(!heap.is_live(Value::bigint(orphan)));
    assert_eq!(heap.get_property(holder, key), Some(Value::TRUE));

    heap.handles().release(handle);
    heap.collect();
    for value in [
        Value::symbol(symbol),
        Value::string(description),
        Value::bigint(big),
        Value::symbol(keyed),
    ] {
        assert!(!heap.is_live(value));
    }
    let zero = heap.alloc_bigint(true, &[0]).unwrap();
    assert!(
        !heap.bigint(zero).negative && heap.bigint(zero).limbs.is_empty(),
        "no negative zero"
    );
}
