use super::*;

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
    let stats = state.collect_cycles().unwrap();
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
