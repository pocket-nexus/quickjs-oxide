use super::*;

#[test]
fn finalized_shapes_unlink_exact_weak_cache_entries() {
    let runtime = Runtime::new();
    let mut objects = Vec::new();
    for index in 0..2_000 {
        let object = runtime.new_object(None).unwrap();
        let key = runtime
            .intern_property_key(&format!("unique-{index}"))
            .unwrap();
        assert!(set_property(&runtime, &object, &key, Value::Int(index)).unwrap());
        objects.push(object);
    }
    assert!(runtime.0.state.borrow().shape_hashes.len() >= objects.len());
    drop(objects);
    runtime.run_gc().unwrap(); // Explicit GC also releases optional prefix roots.
    let state = runtime.0.state.borrow();
    assert!(state.shape_cache.is_empty());
    assert!(state.shape_hashes.is_empty());
}

#[test]
fn delayed_shape_cleanup_does_not_unlink_reused_slot_or_its_atoms() {
    let runtime = Runtime::new();
    let key = runtime
        .intern_property_key("delayed-shape-cleanup")
        .unwrap();
    let atom = key.atom();
    let entry = crate::engine::object::shape::ShapeEntry {
        atom: AtomIdx::from_raw(atom.raw()),
        flags: PropertyFlags::data(true, true, true),
    };
    let mut state = runtime.0.state.borrow_mut();
    let old = state.get_or_create_shape(None, &[entry]).unwrap();
    let cleanup = state.heap.release_shape(old).unwrap();
    assert_eq!(cleanup.finalized_shape_ids, vec![old]);

    let replacement = state.get_or_create_shape(None, &[entry]).unwrap();
    assert_eq!(replacement.index, old.index);
    assert_ne!(replacement.generation, old.generation);
    state.apply_cleanup(cleanup).unwrap();
    assert!(state.shape_is_canonical(replacement));
    assert!(state.heap.shape(replacement).is_ok());
    assert!(state.atoms.resolve(atom).is_ok());

    let cleanup = state.heap.release_shape(replacement).unwrap();
    state.apply_cleanup(cleanup).unwrap();
    assert!(state.shape_cache.is_empty());
    assert!(state.shape_hashes.is_empty());
}

#[test]
fn canonical_intermediate_shape_survives_a_later_unique_append() {
    let runtime = Runtime::new();
    let x = runtime.intern_property_key("intermediate-x").unwrap();
    let y = runtime.intern_property_key("intermediate-y").unwrap();
    let first = runtime.new_object(None).unwrap();
    assert!(set_property(&runtime, &first, &x, Value::Int(1)).unwrap());
    assert!(set_property(&runtime, &first, &y, Value::Int(2)).unwrap());
    let first_shape = runtime
        .0
        .state
        .borrow()
        .heap
        .object(first.object_id())
        .unwrap()
        .shape;

    let second = runtime.new_object(None).unwrap();
    assert!(set_property(&runtime, &second, &x, Value::Int(3)).unwrap());
    assert!(set_property(&runtime, &second, &y, Value::Int(4)).unwrap());
    let state = runtime.0.state.borrow();
    assert_eq!(
        state.heap.object(second.object_id()).unwrap().shape,
        first_shape,
        "literal-built objects must converge on one canonical two-property shape"
    );
    assert_eq!(state.heap.shape(first_shape).unwrap().entries().len(), 2);
}

#[test]
fn unique_shape_append_never_mutates_a_shared_shape() {
    let runtime = Runtime::new();
    let first = runtime.new_object(None).unwrap();
    let second = runtime.new_object(None).unwrap();
    let shared_keys = (0..crate::engine::object::properties::MIN_UNIQUE_SHAPE_APPEND_ENTRIES)
        .map(|index| {
            runtime
                .intern_property_key(&format!("shared-{index}"))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let b = runtime.intern_property_key("unique-b").unwrap();
    let c = runtime.intern_property_key("unique-c").unwrap();

    for key in &shared_keys {
        assert!(set_property(&runtime, &first, key, Value::Int(1)).unwrap());
        assert!(set_property(&runtime, &second, key, Value::Int(2)).unwrap());
    }
    let shared_shape = {
        let state = runtime.0.state.borrow();
        let first_shape = state.heap.object(first.object_id()).unwrap().shape;
        let second_shape = state.heap.object(second.object_id()).unwrap().shape;
        assert_eq!(first_shape, second_shape);
        assert_eq!(state.heap.shape_strong_count(first_shape), Ok(2));
        first_shape
    };

    assert!(set_property(&runtime, &first, &b, Value::Int(3)).unwrap());
    let unique_shape = runtime
        .0
        .state
        .borrow()
        .heap
        .object(first.object_id())
        .unwrap()
        .shape;
    assert_ne!(unique_shape, shared_shape);
    assert!(
        runtime.0.state.borrow().shape_is_canonical(unique_shape),
        "the successor of a shared shape enters the canonical cache"
    );
    assert!(set_property(&runtime, &first, &c, Value::Int(4)).unwrap());
    let state = runtime.0.state.borrow();
    let final_shape = state.heap.object(first.object_id()).unwrap().shape;
    assert_ne!(
        final_shape, unique_shape,
        "retained prefixes remain immutable"
    );
    assert_eq!(
        state.heap.shape(unique_shape).unwrap().entries().len(),
        shared_keys.len() + 1
    );
    assert!(
        state.shape_is_canonical(unique_shape),
        "the retained prefix remains canonical"
    );
    assert_eq!(
        state.heap.object(second.object_id()).unwrap().shape,
        shared_shape,
        "mutating the unique successor must not alter the shared predecessor"
    );
    assert_eq!(
        state
            .heap
            .shape(shared_shape)
            .unwrap()
            .entries()
            .iter()
            .map(|entry| state.atoms.brand(entry.atom).unwrap())
            .collect::<Vec<_>>(),
        shared_keys
            .iter()
            .map(PropertyKey::atom)
            .collect::<Vec<_>>()
    );
    let mut unique_atoms = shared_keys
        .iter()
        .map(PropertyKey::atom)
        .collect::<Vec<_>>();
    unique_atoms.extend([b.atom(), c.atom()]);
    assert_eq!(
        state
            .heap
            .shape(final_shape)
            .unwrap()
            .entries()
            .iter()
            .map(|entry| state.atoms.brand(entry.atom).unwrap())
            .collect::<Vec<_>>(),
        unique_atoms
    );
}

#[test]
fn unique_shape_append_preserves_key_categories_and_readd_order() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let fillers = (0..5)
        .map(|index| {
            runtime
                .intern_property_key(&format!("unique-fill-{index}"))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let beta = runtime.intern_property_key("unique-beta").unwrap();
    let alpha = runtime.intern_property_key("unique-alpha").unwrap();
    let index = runtime.intern_property_key("2").unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("unique-symbol")))
        .unwrap();
    let symbol_key = PropertyKey::from(&symbol);
    for key in &fillers {
        assert!(set_property(&runtime, &object, key, Value::Int(0)).unwrap());
    }
    for key in [&beta, &symbol_key, &index, &alpha] {
        assert!(set_property(&runtime, &object, key, Value::Int(1)).unwrap());
    }
    let mut expected = vec![index.clone()];
    expected.extend(fillers.iter().cloned());
    expected.extend([beta.clone(), alpha.clone(), symbol_key.clone()]);
    assert_eq!(runtime.own_property_keys(&object).unwrap(), expected);

    assert!(runtime.delete_property(&object, &beta).unwrap());
    assert!(set_property(&runtime, &object, &beta, Value::Int(2)).unwrap());
    let mut expected = vec![index];
    expected.extend(fillers);
    expected.extend([alpha, beta, symbol_key]);
    assert_eq!(runtime.own_property_keys(&object).unwrap(), expected);
}

#[test]
fn unique_shape_append_releases_its_key_atom_with_the_final_object() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let seeds = (0..crate::engine::object::properties::MIN_UNIQUE_SHAPE_APPEND_ENTRIES)
        .map(|index| {
            runtime
                .intern_property_key(&format!("unique-append-seed-{index}"))
                .unwrap()
        })
        .collect::<Vec<_>>();
    let key = runtime
        .intern_property_key("unique-append-final-key")
        .unwrap();
    let atom = key.atom();
    for seed in &seeds {
        assert!(set_property(&runtime, &object, seed, Value::Int(1)).unwrap());
    }
    assert!(set_property(&runtime, &object, &key, Value::Int(2)).unwrap());
    drop(seeds);
    drop(key);
    assert!(
        runtime.0.state.borrow().atoms.resolve(atom).is_ok(),
        "the in-place shape must own its appended key atom"
    );

    drop(object);
    runtime.run_gc().unwrap();
    assert!(
        runtime.0.state.borrow().atoms.resolve(atom).is_err(),
        "final shape collection must release its appended key atom"
    );
}

#[test]
fn failed_unique_shape_append_restores_cache_and_atom_ownership() {
    let runtime = Runtime::new();
    let owner = runtime.new_object(None).unwrap();
    let stale = runtime.new_object(None).unwrap();
    let stale_id = stale.object_id();
    drop(stale);
    let key = runtime.intern_property_key("failed-unique-append").unwrap();
    let atom = key.atom();

    let mut state = runtime.0.state.borrow_mut();
    let shape = state.heap.object(owner.object_id()).unwrap().shape;
    assert_eq!(state.heap.shape_strong_count(shape), Ok(1));
    let before_ref_count = state.atoms.resolve(atom).unwrap().ref_count;
    assert!(state.shape_is_canonical(shape));

    assert!(matches!(
        state.append_unique_layout(
            owner.object_id(),
            atom,
            PropertyFlags::data(true, true, true),
            PropertySlot::Data(RawValue::Object(stale_id)),
        ),
        Err(RuntimeError::Heap(HeapError::Stale { .. }))
    ));
    assert_eq!(
        state.atoms.resolve(atom).unwrap().ref_count,
        before_ref_count,
        "a rejected append must roll back the shape's tentative atom root"
    );
    assert!(state.heap.shape(shape).unwrap().entries().is_empty());
    assert!(state.shape_is_canonical(shape));
}

#[test]
fn append_edges_are_weak_and_unlinked_on_mutation_and_collection() {
    let runtime = Runtime::new();
    let mut state = runtime.0.state.borrow_mut();
    let atom = state.atoms.intern_static("transition-key").unwrap();
    let parent = state.get_or_create_shape(None, &[]).unwrap();
    let entry = crate::engine::object::shape::ShapeEntry {
        atom: AtomIdx::from_raw(atom.raw()),
        flags: crate::engine::object::shape::PropertyFlags::data(true, true, true),
    };
    let first = state.append_transition(parent, entry).unwrap();
    let second = state.append_transition(parent, entry).unwrap();
    assert_eq!(first, second);
    assert_eq!(state.shape_transitions[&parent].len(), 1);
    for shape in [first, second] {
        let cleanup = state.heap.release_shape(shape).unwrap();
        state.apply_cleanup(cleanup).unwrap();
    }
    assert!(!state.shape_transitions.contains_key(&parent));
    assert!(state.shape_transition_parents.is_empty());
    let successor = state.append_transition(parent, entry).unwrap();
    state.unlink_shape_transitions(parent);
    assert!(state.shape_transitions.is_empty());
    assert!(state.shape_transition_parents.is_empty());
    for shape in [successor, parent] {
        let cleanup = state.heap.release_shape(shape).unwrap();
        state.apply_cleanup(cleanup).unwrap();
    }
}

#[test]
fn repeated_append_transition_keeps_one_reverse_edge() {
    let runtime = Runtime::new();
    let mut state = runtime.0.state.borrow_mut();
    let atom = state.atoms.intern_static("repeated-transition").unwrap();
    let parent = state.get_or_create_shape(None, &[]).unwrap();
    let entry = crate::engine::object::shape::ShapeEntry {
        atom: AtomIdx::from_raw(atom.raw()),
        flags: PropertyFlags::data(true, true, true),
    };
    let target = state.append_transition(parent, entry).unwrap();
    let initial_capacity = state.shape_transition_parents[&target].capacity();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    for _ in 0..10_000 {
        let repeated = state.append_transition(parent, entry).unwrap();
        assert_eq!(repeated, target);
        let cleanup = state.heap.release_shape(repeated).unwrap();
        state.apply_cleanup(cleanup).unwrap();
    }
    #[cfg(feature = "profiling")]
    assert_eq!(
        profile
            .snapshot()
            .owned_execution_events
            .get("shape_transition_duplicate_avoided")
            .copied(),
        Some(10_000),
    );
    assert_eq!(state.shape_transitions[&parent].len(), 1);
    assert_eq!(
        state.shape_transition_parents[&target],
        vec![(parent, entry)]
    );
    assert_eq!(
        state.shape_transition_parents[&target].capacity(),
        initial_capacity
    );
    assert_eq!(state.heap.shape_strong_count(target), Ok(1));
    state.unlink_shape_transitions(parent);
    assert!(state.shape_transitions.is_empty());
    assert!(state.shape_transition_parents.is_empty());
    for shape in [target, parent] {
        let cleanup = state.heap.release_shape(shape).unwrap();
        state.apply_cleanup(cleanup).unwrap();
    }
}

#[test]
fn rebound_append_transition_survives_delayed_old_target_cleanup() {
    let runtime = Runtime::new();
    let mut state = runtime.0.state.borrow_mut();
    let atom = state.atoms.intern_static("rebound-transition").unwrap();
    let parent = state.get_or_create_shape(None, &[]).unwrap();
    let entry = crate::engine::object::shape::ShapeEntry {
        atom: AtomIdx::from_raw(atom.raw()),
        flags: PropertyFlags::data(true, true, true),
    };
    let old = state.append_transition(parent, entry).unwrap();
    let delayed = state.heap.release_shape(old).unwrap();
    assert!(state.heap.shape(old).is_err());
    // The old generation's forward/reverse entries still exist until cleanup.
    let replacement = state.append_transition(parent, entry).unwrap();
    assert_eq!(old.index, replacement.index);
    assert_ne!(old.generation, replacement.generation);
    assert!(!state.shape_transition_parents.contains_key(&old));
    assert_eq!(
        state.shape_transition_parents[&replacement],
        vec![(parent, entry)]
    );
    state.apply_cleanup(delayed).unwrap();
    assert_eq!(state.shape_transitions[&parent][&entry], replacement);
    assert_eq!(state.canonical_successor(parent, entry), Some(replacement));
    let cleanup = state.heap.release_shape(replacement).unwrap();
    state.apply_cleanup(cleanup).unwrap();
    assert!(state.shape_transitions.is_empty());
    assert!(state.shape_transition_parents.is_empty());
    let cleanup = state.heap.release_shape(parent).unwrap();
    state.apply_cleanup(cleanup).unwrap();
}
