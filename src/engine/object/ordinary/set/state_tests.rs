//! Raw domains are protected by the current lease or an explicit boundary.
use super::*;
use crate::engine::{
    atom::AtomIdx,
    heap::RawId,
    object::{AccessorValue, OrdinaryPropertyDescriptor},
};

fn target(runtime: &Runtime, context: &mut crate::engine::api::Context, source: &str) -> ObjectRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("object fixture")
    };
    assert!(object.belongs_to(runtime));
    object
}
fn raw_slot(state: &RuntimeState, object: ObjectId, atom: Atom) -> RawValue {
    let data = state.heap.object(object).unwrap();
    let slot = state
        .heap
        .shape(data.shape)
        .unwrap()
        .find(AtomIdx::from_raw(atom.raw()))
        .unwrap();
    let crate::engine::heap::PropertySlot::Data(value) = &data.slots[slot as usize] else {
        panic!("data slot")
    };
    value.clone()
}

#[test]
fn borrowed_local_set_does_not_promote_target_or_key_header_roles_at_maximum() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = target(&runtime, &mut context, "({borrowed_local_write:1})");
    let key = runtime.intern_property_key("borrowed_local_write").unwrap();
    let receiver = runtime
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let rc = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let object_count = state.heap.object_strong_count(object.object_id()).unwrap();
    let atom_count = state.atoms.resolve(key.atom()).unwrap().ref_count.unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object.object_id()), u32::MAX);
    state
        .atoms
        .set_ref_count_for_test(AtomIdx::from_raw(key.atom().raw()), u32::MAX);
    let result = state.start_set_borrowed(
        &runtime.0.poisoned,
        Some(context.realm),
        object.object_id(),
        key.atom(),
        JsValue::Int(42),
        receiver,
    );
    let counts = (
        state.heap.object_strong_count(object.object_id()),
        state.atoms.resolve(key.atom()).unwrap().ref_count,
    );
    // Heap MAX is immortal; restore the actual remaining public owner after
    // the raw receiver role was semantically retired. Atom MAX is checked.
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object.object_id()), object_count - 1);
    state
        .atoms
        .set_ref_count_for_test(AtomIdx::from_raw(key.atom().raw()), atom_count);
    assert!(matches!(
        result.unwrap(),
        SetProgress::Complete(SetAction::Complete)
    ));
    assert_eq!(counts, (Ok(u32::MAX), Some(u32::MAX)));
    assert!(matches!(
        raw_slot(&state, object.object_id(), key.atom()),
        RawValue::Int(42)
    ));
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), rc);
    assert!(!runtime.is_poisoned());
}

#[test]
fn genuine_value_publication_refusal_keeps_old_slot_and_retires_domain_inputs() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = target(&runtime, &mut context, "({x:7})");
    let value = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    let input = runtime
        .dup_jsvalue(&JsValue::Object(value.object_id()))
        .unwrap();
    let receiver = runtime
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let original = state.heap.object_strong_count(value.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(value.object_id()), u32::MAX);
    let result = state.start_set_borrowed(
        &runtime.0.poisoned,
        Some(context.realm),
        object.object_id(),
        key.atom(),
        input,
        receiver,
    );
    let maximum = state.heap.object_strong_count(value.object_id());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(value.object_id()), original - 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(
            crate::engine::heap::HeapError::Overflow { .. }
        ))
    ));
    assert_eq!(maximum, Ok(u32::MAX));
    assert!(matches!(
        raw_slot(&state, object.object_id(), key.atom()),
        RawValue::Int(7)
    ));
    assert_eq!(state.heap.object_strong_count(object.object_id()), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn selected_setter_checked_callee_refusal_precedes_any_callback_input_transfer() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let setter = target(
        &runtime,
        &mut context,
        "(function(v){throw 'must not run'})",
    );
    let object = target(&runtime, &mut context, "({})");
    let key = runtime.intern_property_key("x").unwrap();
    let callable = runtime.as_callable(&setter).unwrap().unwrap();
    runtime
        .define_own_property(
            &object,
            &key,
            &OrdinaryPropertyDescriptor {
                set: DescriptorField::Present(AccessorValue::Callable(callable)),
                configurable: DescriptorField::Present(true),
                ..OrdinaryPropertyDescriptor::new()
            },
        )
        .unwrap();
    let value = runtime.new_object(None).unwrap();
    let input = runtime
        .dup_jsvalue(&JsValue::Object(value.object_id()))
        .unwrap();
    let receiver = runtime
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let count = state.heap.object_strong_count(setter.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
    let result = state.start_set_borrowed(
        &runtime.0.poisoned,
        Some(context.realm),
        object.object_id(),
        key.atom(),
        input,
        receiver,
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), count);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(
            crate::engine::heap::HeapError::Overflow { .. }
        ))
    ));
    assert_eq!(state.heap.object_strong_count(value.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(object.object_id()), Ok(1));
    assert!(state.active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}

#[test]
fn raw_proxy_wait_abandonment_coordinates_busy_state_without_runtime_owner_or_poison() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = target(&runtime, &mut context, "new Proxy({}, {})").into_handle();
    let value = runtime.new_object(None).unwrap().into_handle();
    let key = runtime.intern_property_key("x").unwrap();
    let rc = std::rc::Rc::strong_count(&runtime.0);
    let step = runtime
        .0
        .state
        .borrow_mut()
        .start_set_borrowed(
            &runtime.0.poisoned,
            Some(context.realm),
            object,
            key.atom(),
            JsValue::Object(value),
            JsValue::Object(object),
        )
        .unwrap();
    assert!(matches!(
        &step,
        SetProgress::Waiting {
            phase: SetWait::Proxy,
            ..
        }
    ));
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), rc);
    {
        let _lease = runtime.0.state.borrow_mut();
        step.retire_at_boundary(&runtime).unwrap();
        assert!(!runtime.is_poisoned());
        assert!(runtime.0.deferred_references.has_pending());
    }
    runtime.drain_deferred_references().unwrap();
    let state = runtime.0.state.borrow();
    assert!(state.heap.object(object).is_err());
    assert!(state.heap.object(value).is_err());
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), rc);
}

#[test]
fn published_slot_cleanup_failure_quarantines_domain_receiver_and_queue_suffix() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = target(&runtime, &mut context, "({x:{}})");
    let value = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("x").unwrap();
    let input = runtime
        .dup_jsvalue(&JsValue::Object(value.object_id()))
        .unwrap();
    let receiver = runtime
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let invalid = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    for id in [invalid, later] {
        state
            .heap
            .queue_release_for_test(RawId::Object(id))
            .unwrap();
    }
    state
        .heap
        .set_strong_count_for_test(RawId::Object(invalid), 1);
    let result = state.start_set_borrowed(
        &runtime.0.poisoned,
        Some(context.realm),
        object.object_id(),
        key.atom(),
        input,
        receiver,
    );
    assert!(matches!(result, Err(RuntimeError::Poisoned)));
    assert!(runtime.is_poisoned());
    assert!(
        matches!(raw_slot(&state, object.object_id(), key.atom()), RawValue::Object(id) if id==value.object_id())
    );
    // One public owner, the still-armed domain input and the published slot.
    assert_eq!(state.heap.object_strong_count(value.object_id()), Ok(3));
    assert_eq!(state.heap.object_strong_count(object.object_id()), Ok(2));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn synchronous_borrowed_boundary_rejects_foreign_key_with_raw_inputs_retired() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let key = foreign.intern_property_key("x").unwrap();
    let value = runtime.new_object(None).unwrap().into_handle();
    let receiver = runtime
        .dup_jsvalue(&JsValue::Object(object.object_id()))
        .unwrap();
    let result = runtime.internal_set_jsvalue(
        context.realm,
        &object,
        &key,
        JsValue::Object(value),
        receiver,
    );
    assert!(matches!(
        result,
        Err(RuntimeError::WrongRuntime("property key"))
    ));
    assert!(runtime.0.state.borrow().heap.object(value).is_err());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(object.object_id()),
        Ok(1)
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn public_set_receiver_conversion_rejection_retires_the_first_converted_value() {
    let runtime = Runtime::new();
    let foreign = Runtime::new();
    let value = runtime.new_object(None).unwrap();
    let id = value.object_id();
    let receiver = foreign.new_object(None).unwrap();
    let receiver_id = receiver.object_id();
    let result = SetStep::prepare_inputs(&runtime, Value::Object(value), Value::Object(receiver));
    assert!(matches!(
        result,
        Err(RuntimeError::WrongRuntime("object root conversion"))
    ));
    assert!(runtime.0.state.borrow().heap.object(id).is_err());
    assert!(foreign.0.state.borrow().heap.object(receiver_id).is_err());
    assert!(!runtime.is_poisoned());
    assert!(!foreign.is_poisoned());
}
