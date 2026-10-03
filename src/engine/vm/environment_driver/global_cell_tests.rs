use super::*;
use crate::engine::{
    code::{
        function::metadata::{
            ClosureSource, ClosureVariable, ClosureVariableKind, ClosureVariableName,
        },
        runtime::PublishedFunctionSnapshot,
    },
    heap::RawId,
    vm::closure::ClosureSlots,
};

fn executable(runtime: &Runtime, atom: crate::engine::atom::Atom) -> PublishedFunctionSnapshot {
    let context = runtime.new_context().expect("create context");
    let mut executable = PublishedFunctionSnapshot::empty_for_test(context.realm);
    executable.closure_variables = vec![ClosureVariable {
        source: ClosureSource::Global,
        name: ClosureVariableName::Atom(atom),
        is_lexical: false,
        is_const: false,
        kind: ClosureVariableKind::Normal,
    }]
    .into();
    executable
}

#[test]
fn global_cell_reads_current_value_and_keeps_output_alive_after_overwrite() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let key = runtime.intern_property_key("cell").unwrap();
    let executable = executable(&runtime, key.atom());
    let roots = ClosureSlots::from(vec![runtime.new_uninitialized_var_ref().unwrap()]);
    for source in [
        "undefined",
        "null",
        "true",
        "42",
        "-0",
        "NaN",
        "'hello'",
        "1n",
        "123456789012345678901234567890n",
        "Symbol('s')",
        "({x:1})",
    ] {
        let public = context.eval(source).unwrap();
        let original = runtime.into_jsvalue(public).unwrap();
        let expected = runtime.dup_jsvalue(&original).unwrap();
        runtime
            .write_var_ref(&roots.get(0).unwrap(), original)
            .unwrap();
        let output = try_read_global_cell(&runtime, &executable, &roots, 0)
            .unwrap()
            .unwrap();
        runtime
            .write_var_ref(&roots.get(0).unwrap(), JsValue::Int(19))
            .unwrap();
        if let (JsValue::Float(a), JsValue::Float(b)) = (&output, &expected) {
            assert_eq!(a.to_bits(), b.to_bits());
        } else {
            assert_eq!(output, expected, "{source}");
        }
        let current = try_read_global_cell(&runtime, &executable, &roots, 0)
            .unwrap()
            .unwrap();
        assert_eq!(current, JsValue::Int(19));
        runtime.release_jsvalue(output).unwrap();
        runtime.release_jsvalue(expected).unwrap();
        runtime.release_jsvalue(current).unwrap();
    }
}

#[test]
fn global_cell_misses_leave_uninitialized_and_pending_cleanup_untouched() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let key = runtime.intern_property_key("cell").unwrap();
    let executable = executable(&runtime, key.atom());
    let roots = ClosureSlots::from(vec![runtime.new_uninitialized_var_ref().unwrap()]);
    assert!(
        try_read_global_cell(&runtime, &executable, &roots, 0)
            .unwrap()
            .is_none()
    );
    runtime
        .write_var_ref(&roots.get(0).unwrap(), JsValue::Int(42))
        .unwrap();
    let state = runtime.0.state.borrow_mut();
    assert!(
        try_read_global_cell(&runtime, &executable, &roots, 0)
            .unwrap()
            .is_none()
    );
    drop(state);
    let garbage = context.new_object().unwrap();
    let state = runtime.0.state.borrow_mut();
    drop(garbage);
    assert!(runtime.0.deferred_references.has_pending());
    drop(state);
    assert!(
        try_read_global_cell(&runtime, &executable, &roots, 0)
            .unwrap()
            .is_none()
    );
    assert!(runtime.0.deferred_references.has_pending());
    runtime
        .write_var_ref(&roots.get(0).unwrap(), JsValue::Int(43))
        .unwrap();
    assert_eq!(
        try_read_global_cell(&runtime, &executable, &roots, 0).unwrap(),
        Some(JsValue::Int(43))
    );
}

#[test]
fn global_cell_checked_retain_preserves_overflow_and_immortal_transition() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let key = runtime.intern_property_key("cell").unwrap();
    let executable = executable(&runtime, key.atom());
    let object = context.new_object().unwrap();
    let id = object.object_id();
    let roots = ClosureSlots::from(vec![
        runtime
            .new_var_ref_rooted(
                Value::Object(object),
                false,
                false,
                ClosureVariableKind::Normal,
            )
            .unwrap(),
    ]);
    let original = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), u32::MAX);
    let failed = try_read_global_cell(&runtime, &executable, &roots, 0);
    let after_failure = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), u32::MAX - 1);
    let near = try_read_global_cell(&runtime, &executable, &roots, 0);
    let after_retain = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    if let Ok(Some(value)) = near {
        runtime.release_jsvalue(value).unwrap();
    }
    let after_release = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), original);
    assert!(
        failed
            .unwrap_err()
            .message()
            .contains("retaining a heap reference")
    );
    assert_eq!(after_failure, u32::MAX);
    assert_eq!(after_retain, u32::MAX);
    assert_eq!(after_release, u32::MAX);
}

#[test]
fn global_cell_rejects_foreign_closure_roots() {
    let runtime = Runtime::new();
    let other = Runtime::new();
    let key = runtime.intern_property_key("cell").unwrap();
    let executable = executable(&runtime, key.atom());
    let roots = ClosureSlots::from(vec![other.new_uninitialized_var_ref().unwrap()]);
    assert!(
        try_read_global_cell(&runtime, &executable, &roots, 0)
            .unwrap_err()
            .message()
            .contains("another runtime")
    );
}

#[test]
fn global_cell_pending_zero_cleanup_misses_without_drain() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let key = runtime.intern_property_key("cell").unwrap();
    let executable = executable(&runtime, key.atom());
    let roots = ClosureSlots::from(vec![
        runtime
            .new_var_ref(JsValue::Int(42), false, false, ClosureVariableKind::Normal)
            .unwrap(),
    ]);
    let garbage = context.new_object().unwrap().into_handle();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .queue_release_for_test(RawId::Object(garbage))
        .unwrap();
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(runtime.0.state.borrow().heap.has_pending_zero_cleanup());
    assert!(
        try_read_global_cell(&runtime, &executable, &roots, 0)
            .unwrap()
            .is_none()
    );
    assert!(runtime.0.state.borrow().heap.has_pending_zero_cleanup());
    runtime.run_gc().unwrap();
    assert_eq!(
        try_read_global_cell(&runtime, &executable, &roots, 0).unwrap(),
        Some(JsValue::Int(42))
    );
}
