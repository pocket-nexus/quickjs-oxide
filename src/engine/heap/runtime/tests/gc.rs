use super::*;
use crate::engine::heap::{GcPolicy, ObjectData, RawId, Shape, WeakCollectionKey};
use crate::engine::object::shape::ShapeEntry;

#[test]
fn gc_weak_atom_cleanup_failure_preserves_error_and_stops_the_sweep() {
    for direct in [false, true] {
        let runtime = Runtime::new();
        let mut state = runtime.0.state.borrow_mut();
        let shape = state
            .heap
            .allocate_shape(Shape::new(None, []).unwrap())
            .unwrap();
        let first_map = state
            .heap
            .allocate_object(ObjectData::weak_map(shape, Vec::new()))
            .unwrap();
        let later_map = state
            .heap
            .allocate_object(ObjectData::weak_map(shape, Vec::new()))
            .unwrap();
        let key = state
            .heap
            .allocate_object(ObjectData::ordinary(shape, Vec::new()))
            .unwrap();
        let later_value = state
            .heap
            .allocate_object(ObjectData::ordinary(shape, Vec::new()))
            .unwrap();
        let weak_key = WeakCollectionKey::Object(key);
        let broken_atom = state.atoms.new_symbol(Some("detached GC value")).unwrap();
        state
            .heap
            .weak_map_set(
                first_map,
                weak_key,
                RawValue::Symbol(AtomIdx::from_raw(broken_atom.raw())),
            )
            .unwrap();
        state
            .heap
            .weak_map_set(later_map, weak_key, RawValue::Object(later_value))
            .unwrap();
        state.heap.release_object(key).unwrap();
        state.heap.release_object(later_value).unwrap();
        // Corrupt a genuinely heap-owned atom, not the GC hook implementation.
        state.atoms.release(broken_atom).unwrap();
        drop(state);

        let result = if direct {
            let mut state = runtime.0.state.borrow_mut();
            state.collect_cycles(&runtime.0.poisoned)
        } else {
            runtime.run_gc()
        };
        assert!(matches!(
            result,
            Err(RuntimeError::Atom(crate::engine::atom::AtomError::UnknownAtom(atom)))
                if atom.raw() == broken_atom.raw()
        ));
        assert!(runtime.is_poisoned());
        let state = runtime.0.state.borrow();
        assert!(
            state
                .heap
                .weak_map_get(first_map, weak_key)
                .unwrap()
                .is_none()
        );
        assert!(matches!(
            state.heap.weak_map_get(later_map, weak_key).unwrap(),
            Some(RawValue::Object(id)) if *id == later_value
        ));
        assert_eq!(state.heap.object_strong_count(later_value), Ok(1));
        drop(state);
        assert!(matches!(runtime.run_gc(), Err(RuntimeError::Poisoned)));
        assert!(matches!(
            runtime.new_object(None),
            Err(RuntimeError::Poisoned)
        ));
    }
}

#[test]
fn gc_finalized_atom_failure_stops_before_later_owned_atoms() {
    let runtime = Runtime::new();
    let mut state = runtime.0.state.borrow_mut();
    let broken = state.atoms.intern("gc-owned-broken").unwrap();
    let later = state.atoms.intern("gc-owned-later").unwrap();
    let shape = state
        .heap
        .allocate_shape(
            Shape::new(
                None,
                [broken, later].map(|atom| ShapeEntry {
                    atom: AtomIdx::from_raw(atom.raw()),
                    flags: PropertyFlags::data(true, true, true),
                }),
            )
            .unwrap(),
        )
        .unwrap();
    let object = state
        .heap
        .allocate_object(ObjectData::ordinary(
            shape,
            vec![
                PropertySlot::Data(RawValue::Undefined),
                PropertySlot::Data(RawValue::Undefined),
            ],
        ))
        .unwrap();
    let cleanup = state
        .heap
        .replace_object_slot(object, 0, PropertySlot::Data(RawValue::Object(object)))
        .unwrap();
    state.apply_cleanup(cleanup).unwrap();
    state.heap.release_shape(shape).unwrap();
    state.heap.release_object(object).unwrap();
    state.atoms.release(broken).unwrap();
    let result = state.collect_cycles(&runtime.0.poisoned);
    assert!(matches!(result, Err(RuntimeError::Atom(_))));
    assert!(runtime.is_poisoned());
    assert!(state.heap.object(object).is_err());
    assert!(state.heap.shape(shape).is_err());
    assert!(state.atoms.is_live(later));
    drop(state);
    assert!(matches!(runtime.run_gc(), Err(RuntimeError::Poisoned)));
}

#[test]
fn gc_applies_retained_prefix_cleanup_before_recoverable_preflight() {
    let runtime = Runtime::new();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let mut context = runtime.new_context().unwrap();
    let registry = runtime
        .into_jsvalue(
            context
                .eval("(()=>{let r=new FinalizationRegistry(()=>{});let k={};r.register(k,17);return r})()")
                .unwrap(),
        )
        .unwrap();
    let JsValue::Object(registry_id) = registry else {
        panic!("registry")
    };
    let mut state = runtime.0.state.borrow_mut();
    let atom = state.atoms.intern("gc-retained-prefix").unwrap();
    let shape = state
        .heap
        .allocate_shape(
            Shape::new(
                None,
                [ShapeEntry {
                    atom: AtomIdx::from_raw(atom.raw()),
                    flags: PropertyFlags::data(true, true, true),
                }],
            )
            .unwrap(),
        )
        .unwrap();
    state.retain_construction_shape(shape).unwrap();
    state.heap.release_shape(shape).unwrap();
    let ObjectPayload::FinalizationRegistry(registry_data) =
        &state.heap.object(registry_id).unwrap().payload
    else {
        panic!("registry payload")
    };
    let callback = registry_data.callback;
    let callback_count = state.heap.object_strong_count(callback).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(callback), u32::MAX);
    assert_eq!(
        state.collect_cycles(&runtime.0.poisoned),
        Err(RuntimeError::Heap(HeapError::Overflow {
            operation: "retaining outgoing heap edges",
        }))
    );
    assert!(!runtime.is_poisoned());
    assert!(!state.atoms.is_live(atom));
    assert!(state.heap.shape(shape).is_err());
    assert_eq!(state.heap.finalization_registry_len(registry_id), Ok(1));
    state
        .heap
        .set_strong_count_for_test(RawId::Object(callback), callback_count);
    drop(state);
    runtime.run_gc().unwrap();
    runtime
        .release_jsvalue(JsValue::Object(registry_id))
        .unwrap();
}

#[test]
fn direct_state_gc_collects_cycles_without_reborrowing_runtime() {
    let runtime = Runtime::new();
    let baseline = runtime.heap_counts().expect("runtime state");
    let object = runtime.new_object(None).unwrap();
    let self_key = runtime.intern_property_key("self").unwrap();
    assert!(
        set_property(
            &runtime,
            &object,
            &self_key,
            Value::Object(object.try_clone().unwrap())
        )
        .unwrap()
    );
    let id = object.into_handle();
    let mut state = runtime.0.state.borrow_mut();
    state.release_jsvalue(JsValue::Object(id)).unwrap();
    assert_eq!(state.heap.object_strong_count(id), Ok(1));
    let stats = state.collect_cycles(&runtime.0.poisoned).unwrap();
    assert_eq!(stats.cleanup.finalized_objects, 1);
    assert_eq!(state.heap.counts().object_nodes, baseline.object_nodes);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn explicit_gc_caps_empty_zero_queue_but_ordinary_releases_reuse_it() {
    let runtime = Runtime::new();
    let baseline = runtime.heap_counts().expect("runtime state");
    let roots: Vec<_> = (0..32).map(|_| runtime.new_object(None).unwrap()).collect();
    let retained_capacity = {
        let mut state = runtime.0.state.borrow_mut();
        state.heap.zero_queue.reserve(8192);
        state.heap.zero_queue.capacity()
    };
    assert!(retained_capacity > 4096);

    drop(roots);
    assert_eq!(
        runtime.heap_counts().expect("runtime state").object_nodes,
        baseline.object_nodes
    );
    assert_eq!(
        runtime.0.state.borrow().heap.zero_queue.capacity(),
        retained_capacity
    );

    runtime.run_gc().unwrap();
    let state = runtime.0.state.borrow();
    assert!(state.heap.zero_queue.is_empty());
    assert!(state.heap.zero_queue.capacity() <= 4096);
}

#[test]
fn explicit_gc_drains_deferred_release_before_trimming_zero_queue() {
    let runtime = Runtime::new();
    let baseline = runtime.heap_counts().expect("runtime state");
    let object = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    state.heap.zero_queue.reserve(8192);
    drop(object);
    assert!(runtime.0.deferred_references.has_pending());
    drop(state);

    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").object_nodes,
        baseline.object_nodes
    );
    assert!(!runtime.0.deferred_references.has_pending());
    let state = runtime.0.state.borrow();
    assert!(state.heap.zero_queue.is_empty());
    assert!(state.heap.zero_queue.capacity() <= 4096);
}

#[test]
fn object_property_cycle_is_collected_only_by_explicit_gc() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let self_key = runtime.intern_property_key("self").unwrap();
    assert!(
        set_property(
            &runtime,
            &object,
            &self_key,
            Value::Object(object.try_clone().expect("duplicate root"))
        )
        .unwrap()
    );
    assert_eq!(
        runtime.heap_counts().expect("runtime state").object_nodes,
        1
    );
    let state = runtime.0.state.borrow_mut();
    drop(object);
    drop(state);
    let stats = runtime.run_gc().unwrap();
    assert_eq!(stats.cleanup.finalized_objects, 1);
    assert_eq!(
        runtime.heap_counts().expect("runtime state").object_nodes,
        0
    );
}

#[test]
fn shape_prototype_property_cycle_keeps_external_root_then_collects() {
    let runtime = Runtime::new();
    let baseline = runtime.heap_counts().expect("runtime state");
    let prototype = runtime.new_object(None).unwrap();
    let object = runtime.new_object(Some(&prototype)).unwrap();
    let back = runtime.intern_property_key("back").unwrap();
    assert!(
        set_property(
            &runtime,
            &prototype,
            &back,
            Value::Object(object.try_clone().expect("duplicate root"))
        )
        .unwrap()
    );
    drop(prototype);

    runtime.run_gc().unwrap();
    let rooted = runtime.heap_counts().expect("runtime state");
    assert!(rooted.object_nodes >= baseline.object_nodes + 2);
    assert!(rooted.shape_nodes >= baseline.shape_nodes + 2);

    drop(object);
    let stats = runtime.run_gc().unwrap();
    assert!(stats.cleanup.finalized_objects >= 2);
    assert!(stats.cleanup.finalized_shapes >= 2);
    let collected = runtime.heap_counts().expect("runtime state");
    assert_eq!(collected.object_nodes, baseline.object_nodes);
    assert_eq!(collected.shape_nodes, baseline.shape_nodes);
}

#[test]
fn named_function_self_capture_cycle_is_collected() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let baseline = runtime.heap_counts().expect("runtime state");
    let closure = context
        .eval(
            "(function() {\
                var child;\
                var owner = function self() {\
                    child = function() { return self; };\
                    return child;\
                };\
                return owner();\
            })()",
        )
        .unwrap();
    let retained = runtime.heap_counts().expect("runtime state");
    assert!(retained.object_nodes >= baseline.object_nodes + 2);
    assert!(retained.var_ref_nodes >= baseline.var_ref_nodes + 2);
    drop(closure);
    assert!(runtime.heap_counts().expect("runtime state").object_nodes > baseline.object_nodes);

    let stats = runtime.run_gc().unwrap();
    assert!(stats.cleanup.finalized_objects >= 2);
    assert!(stats.cleanup.finalized_var_refs >= 2);
    let collected = runtime.heap_counts().expect("runtime state");
    assert_eq!(collected.object_nodes, baseline.object_nodes);
    assert_eq!(collected.var_ref_nodes, baseline.var_ref_nodes);
    assert_eq!(
        collected.function_bytecode_nodes,
        baseline.function_bytecode_nodes
    );
}

#[test]
fn exceptional_vm_exit_releases_local_frame_roots_immediately() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let mut context = runtime.new_context().expect("create context");
    let function = eval_callable(
        &runtime,
        &mut context,
        "(function(root){ let local=root; return 1n + 1; })",
    );
    let before = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(object.object_id())
        .unwrap();
    assert!(
        context
            .call(
                &function,
                Value::Undefined,
                &[Value::Object(object.try_clone().expect("duplicate root"))]
            )
            .is_err()
    );
    drop(context.take_exception().unwrap());
    let after = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(object.object_id())
        .unwrap();
    assert_eq!(after, before);
}

#[test]
fn drops_during_runtime_borrow_are_deferred_to_the_next_safe_point() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("queued").unwrap();
    let state = runtime.0.state.borrow_mut();
    drop(object);
    drop(key);
    assert_eq!(runtime.0.deferred_references.borrow().len(), 2);
    drop(state);

    let context = runtime.new_context().expect("create context");
    assert!(runtime.0.deferred_references.borrow().is_empty());
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(context);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").object_nodes,
        0
    );
}
