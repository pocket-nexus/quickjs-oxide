use super::*;
use crate::engine::{
    api::Runtime,
    code::function::metadata::{ClosureSource, ClosureVariableName},
    heap::{HeapError, RawId, RawValue, runtime::DeferredRefOp},
    value::{JsString, JsValue},
};

fn descriptor(kind: ClosureVariableKind, is_const: bool) -> ClosureVariable {
    ClosureVariable {
        source: ClosureSource::ParentLocal(0),
        name: ClosureVariableName::None,
        is_lexical: true,
        is_const,
        kind,
    }
}

#[test]
fn captured_local_uses_parent_metadata_and_relay_preserves_import_view() {
    let runtime = Runtime::new();
    let value = runtime.new_object(None).unwrap().into_handle();
    let mut binding = FrameBinding::Direct(JsValue::Object(value));
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let child = descriptor(ClosureVariableKind::ModuleImportView, true);
    let cell = capture_local_binding(
        &mut state,
        &runtime.0.poisoned,
        &mut binding,
        VariableDefinition {
            name: None,
            is_lexical: true,
            is_const: false,
            is_parameter_initializer: false,
            kind: ClosureVariableKind::Normal,
        },
        child,
    )
    .unwrap();
    assert_eq!(
        state.heap.var_ref(cell).unwrap().kind,
        ClosureVariableKind::Normal
    );
    assert!(!state.heap.var_ref(cell).unwrap().is_const);
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    let relay =
        capture_frame_binding(&mut state, &runtime.0.poisoned, &mut binding, child).unwrap();
    assert_eq!(relay, cell);
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(3));
    assert_eq!(state.heap.object_strong_count(value), Ok(1));
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    assert!(!runtime.0.deferred_references.has_pending());
    state.release_var_ref_handle(cell).unwrap();
    state.release_var_ref_handle(relay).unwrap();
    release_frame_binding_in_state(&mut state, binding).unwrap();
    assert!(state.heap.var_ref(cell).is_err());
    assert!(state.heap.object(value).is_err());
}

#[test]
fn reused_cell_checked_overflow_keeps_binding_and_owner_recoverable() {
    let runtime = Runtime::new();
    let mut binding = FrameBinding::Uninitialized;
    let desc = descriptor(ClosureVariableKind::Normal, false);
    let mut state = runtime.0.state.borrow_mut();
    let cell = capture_frame_binding(&mut state, &runtime.0.poisoned, &mut binding, desc).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::VarRef(cell), u32::MAX);
    let result = capture_frame_binding(&mut state, &runtime.0.poisoned, &mut binding, desc);
    state.heap.set_strong_count_for_test(RawId::VarRef(cell), 2);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(matches!(binding, FrameBinding::Captured(id) if id == cell));
    assert!(matches!(
        state.raw_var_ref_value(cell).unwrap(),
        RawValue::Uninitialized
    ));
    assert!(!runtime.is_poisoned());
    state.release_var_ref_handle(cell).unwrap();
    release_frame_binding_in_state(&mut state, binding).unwrap();
}

#[test]
fn private_field_capture_uses_current_state_and_retires_original_atom_edge() {
    let runtime = Runtime::new();
    let name = runtime
        .new_private_name(JsString::from_static("field"))
        .unwrap();
    let atom = name.atom();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let index = state.atoms.unbrand(atom).unwrap();
    state.atoms.retain_index(index).unwrap();
    let mut binding = FrameBinding::Private(index);
    let cell = capture_frame_binding(
        &mut state,
        &runtime.0.poisoned,
        &mut binding,
        descriptor(ClosureVariableKind::PrivateField, true),
    )
    .unwrap();
    assert!(matches!(state.raw_var_ref_value(cell).unwrap(), RawValue::Private(id) if id == index));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(2));
    assert_eq!(state.heap.var_ref_strong_count(cell), Ok(2));
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    assert!(!runtime.0.deferred_references.has_pending());
    state.release_var_ref_handle(cell).unwrap();
    release_frame_binding_in_state(&mut state, binding).unwrap();
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
}

#[test]
fn capture_adapter_admits_before_moving_direct_owner() {
    let runtime = Runtime::new();
    let stale = runtime.new_object(None).unwrap().into_handle();
    runtime.release_jsvalue(JsValue::Object(stale)).unwrap();
    let value = runtime.new_object(None).unwrap().into_handle();
    let mut binding = FrameBinding::Direct(JsValue::Object(value));
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(stale));
    let result = crate::engine::vm::bindings::capture_frame_binding(
        &runtime,
        &mut binding,
        descriptor(ClosureVariableKind::Normal, false),
    );
    assert!(result.is_err());
    assert!(matches!(binding, FrameBinding::Direct(JsValue::Object(id)) if id == value));
    assert!(runtime.is_poisoned());
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(value),
        Ok(1)
    );
}
