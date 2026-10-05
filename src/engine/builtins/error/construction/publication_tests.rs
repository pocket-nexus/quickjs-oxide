use super::*;
use crate::engine::atom::{Atom, AtomIdx, pinned::PinnedAtom};
use crate::engine::heap::runtime::owned_values::OwnedValuesGuard;
use crate::engine::heap::{HeapError, PropertySlot, RawId};
use crate::engine::value::JsString;
use crate::engine::vm::frames::ExplicitBacktraceLocation;
use crate::source::LineColumn;

fn queue_error() -> RuntimeError {
    RuntimeError::Heap(HeapError::Invariant(
        "finalization count disagrees with its queue/cycle state",
    ))
}

fn inject_older_invalid_zero_queue(state: &mut RuntimeState, first: ObjectId, later: ObjectId) {
    state
        .heap
        .queue_release_for_test(RawId::Object(first))
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(later))
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(first), 1);
}

fn error_prototype(state: &RuntimeState, realm: ContextId) -> ObjectId {
    state.heap.context(realm).unwrap().native_error_prototypes[NativeErrorKind::Type.index()]
        .unwrap()
}

fn empty_error(state: &mut RuntimeState, prototype: ObjectId) -> ObjectId {
    state
        .allocate_object_with_layout(Some(prototype), &[], Vec::new(), ObjectData::error)
        .unwrap()
}

fn message_string(state: &RuntimeState, object: ObjectId) -> crate::engine::heap::StringId {
    let object = state.heap.object(object).unwrap();
    let key = state.pinned_atoms.get(PinnedAtom::Message);
    let slot = state
        .heap
        .shape(object.shape)
        .unwrap()
        .find(AtomIdx::from_raw(key.raw()))
        .unwrap() as usize;
    let PropertySlot::Data(RawValue::String(string)) = object.slots[slot] else {
        panic!("published Error message");
    };
    string
}

fn assert_quarantined_suffix(state: &RuntimeState, later: ObjectId, suffix: ObjectId, atom: Atom) {
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.atoms.resolve(atom).unwrap().ref_count, Some(1));
}

#[test]
fn native_error_allocation_cleanup_failure_quarantines_prototype_and_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = error_prototype(&state, context.realm);
    let warm = empty_error(&mut state, prototype);
    let shape = state.heap.object(warm).unwrap().shape;
    let shape_before = state.heap.shape_strong_count(shape).unwrap();
    let prototype_before = state.heap.object_strong_count(prototype).unwrap();
    let before = state.heap.counts();
    let atom = state.atoms.new_symbol(Some("allocation-suffix")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    inject_older_invalid_zero_queue(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix), JsValue::Symbol(index)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.new_native_error_without_backtrace_from_message(
                &runtime.0.poisoned,
                context.realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8("unreached"),
            ),
            Err(queue_error())
        );
        assert!(runtime.0.poisoned.get());
    }
    assert_eq!(state.heap.counts().object_nodes, before.object_nodes + 1);
    assert_eq!(state.heap.counts().string_nodes, before.string_nodes);
    assert_eq!(state.heap.shape_strong_count(shape), Ok(shape_before + 1));
    assert_eq!(
        state.heap.object_strong_count(prototype),
        Ok(prototype_before + 1)
    );
    assert_quarantined_suffix(&state, later, suffix, atom);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn native_error_message_publication_failure_quarantines_all_enclosing_owners() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = error_prototype(&state, context.realm);
    let object = empty_error(&mut state, prototype);
    let peer = empty_error(&mut state, prototype);
    let shape = state.heap.object(object).unwrap().shape;
    assert_eq!(state.heap.object(peer).unwrap().shape, shape);
    assert!(state.heap.shape_strong_count(shape).unwrap() >= 2);
    let atom = state.atoms.new_symbol(Some("message-suffix")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    state.heap.retain_object(prototype).unwrap();
    inject_older_invalid_zero_queue(&mut state, first, later);
    let prototype_at_failure;
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix), JsValue::Symbol(index)],
        );
        let (state, _) = suffix_owner.parts();
        let mut prototype_owner =
            OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Object(prototype));
        let (state, _) = prototype_owner.parts();
        let mut object_owner =
            OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Object(object));
        let (state, _) = object_owner.parts();
        assert_eq!(
            state.initialize_native_error_message(
                &runtime.0.poisoned,
                object,
                NativeErrorMessage::from_utf8("published"),
            ),
            Err(queue_error())
        );
        assert!(runtime.0.poisoned.get());
        prototype_at_failure = state.heap.object_strong_count(prototype).unwrap();
    }
    let string = message_string(&state, object);
    assert_eq!(
        state.heap.string(string).unwrap(),
        &JsString::from_static("published")
    );
    assert_eq!(state.heap.strong_count(RawId::String(string)), Ok(2));
    assert_eq!(state.heap.object_strong_count(object), Ok(1));
    assert_eq!(
        state.heap.object_strong_count(prototype),
        Ok(prototype_at_failure)
    );
    assert_eq!(state.heap.object_strong_count(peer), Ok(1));
    assert_quarantined_suffix(&state, later, suffix, atom);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn fresh_backtrace_publication_failure_preserves_field_atom_and_temporary_owners() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let filename_key = runtime.intern_property_key("fileName").unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = error_prototype(&state, context.realm);
    let [object, peer] = [(); 2].map(|()| {
        state
            .new_native_error_without_backtrace_from_message(
                &runtime.0.poisoned,
                context.realm,
                NativeErrorKind::Type,
                NativeErrorMessage::from_utf8("message"),
            )
            .unwrap()
    });
    let shape = state.heap.object(object).unwrap().shape;
    assert_eq!(state.heap.object(peer).unwrap().shape, shape);
    assert!(state.heap.shape_strong_count(shape).unwrap() >= 2);
    let field_atom_before = state
        .atoms
        .resolve(filename_key.atom())
        .unwrap()
        .ref_count
        .unwrap();
    let atom = state.atoms.new_symbol(Some("backtrace-suffix")).unwrap();
    let index = state.atoms.unbrand(atom).unwrap();
    state.heap.retain_object(prototype).unwrap();
    inject_older_invalid_zero_queue(&mut state, first, later);
    let prototype_at_failure;
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix), JsValue::Symbol(index)],
        );
        let (state, _) = suffix_owner.parts();
        let mut prototype_owner =
            OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Object(prototype));
        let (state, _) = prototype_owner.parts();
        let mut object_owner =
            OwnedValueGuard::new(state, &runtime.0.poisoned, JsValue::Object(object));
        let (state, _) = object_owner.parts();
        assert_eq!(
            state.complete_fresh_native_error_backtrace(
                &runtime.0.poisoned,
                object,
                false,
                Some(ExplicitBacktraceLocation {
                    filename: JsString::from_static("published.js"),
                    position: LineColumn::new(2, 3),
                }),
            ),
            Err(queue_error())
        );
        assert!(runtime.0.poisoned.get());
        prototype_at_failure = state.heap.object_strong_count(prototype).unwrap();
    }
    let data = state.heap.object(object).unwrap();
    let shape = state.heap.shape(data.shape).unwrap();
    assert_eq!(data.slots.len(), 2);
    assert_eq!(shape.entries()[1].atom.raw(), filename_key.atom().raw());
    let PropertySlot::Data(RawValue::String(string)) = data.slots[1] else {
        panic!("published filename");
    };
    assert_eq!(
        state.heap.string(string).unwrap(),
        &JsString::from_static("published.js")
    );
    assert_eq!(state.heap.strong_count(RawId::String(string)), Ok(2));
    assert_eq!(
        state
            .heap
            .strong_count(RawId::String(message_string(&state, object))),
        Ok(1)
    );
    assert_eq!(state.heap.object_strong_count(object), Ok(2));
    assert_eq!(
        state.heap.object_strong_count(prototype),
        Ok(prototype_at_failure)
    );
    assert_eq!(
        state.atoms.resolve(filename_key.atom()).unwrap().ref_count,
        Some(field_atom_before + 2)
    );
    assert_quarantined_suffix(&state, later, suffix, atom);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn native_error_message_rejection_is_recoverable_and_reclaims_its_producer() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = error_prototype(&state, context.realm);
    let object = empty_error(&mut state, prototype);
    state.heap.set_object_extensible(object, false).unwrap();
    let before = state.heap.counts();
    assert_eq!(
        state.initialize_native_error_message(
            &runtime.0.poisoned,
            object,
            NativeErrorMessage::from_utf8("rejected"),
        ),
        Err(RuntimeError::Invariant(
            "native Error message definition was rejected"
        ))
    );
    assert!(!runtime.0.poisoned.get());
    assert_eq!(state.heap.counts().string_nodes, before.string_nodes);
    assert!(state.heap.object(object).unwrap().slots.is_empty());
    state.heap.set_object_extensible(object, true).unwrap();
    state
        .initialize_native_error_message(
            &runtime.0.poisoned,
            object,
            NativeErrorMessage::from_utf8("retry"),
        )
        .unwrap();
    assert_eq!(
        state
            .heap
            .strong_count(RawId::String(message_string(&state, object))),
        Ok(1)
    );
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
        .unwrap();
    assert!(!runtime.0.poisoned.get());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn native_error_message_slot_retain_overflow_is_recoverable_before_publication() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let prototype = error_prototype(&state, context.realm);
    let object = empty_error(&mut state, prototype);
    let peer = empty_error(&mut state, prototype);
    let string = state
        .heap
        .allocate_string(JsString::from_static("overflow"))
        .unwrap();
    let key = state.pinned_atoms.get(PinnedAtom::Message);
    state
        .heap
        .set_strong_count_for_test(RawId::String(string), u32::MAX);
    let result = state.define_raw_property_with_poison(
        &runtime.0.poisoned,
        object,
        key,
        &PropertyDescriptor {
            value: Some(RawValue::String(string)),
            writable: Some(true),
            enumerable: Some(false),
            configurable: Some(true),
            ..PropertyDescriptor::new()
        },
    );
    state
        .heap
        .set_strong_count_for_test(RawId::String(string), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(!runtime.0.poisoned.get());
    assert!(state.heap.object(object).unwrap().slots.is_empty());
    assert_eq!(state.heap.strong_count(RawId::String(string)), Ok(1));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::String(string))
        .unwrap();
    state
        .initialize_native_error_message(
            &runtime.0.poisoned,
            object,
            NativeErrorMessage::from_utf8("retry"),
        )
        .unwrap();
    assert_eq!(
        state
            .heap
            .strong_count(RawId::String(message_string(&state, object))),
        Ok(1)
    );
    for object in [object, peer] {
        state
            .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(object))
            .unwrap();
    }
    assert!(!runtime.0.poisoned.get());
    assert!(!runtime.0.deferred_references.has_pending());
}
