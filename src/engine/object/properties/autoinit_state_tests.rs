use super::*;
use crate::engine::heap::runtime::{DeferredRefOp, owned_values::OwnedValuesGuard};
use crate::engine::heap::{ObjectKind, RawId};

fn install_lazy(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    object: ObjectId,
    key: Atom,
    initializer: AutoInitProperty,
) {
    state
        .replace_layout_with_poison(
            poisoned,
            object,
            None,
            &[ShapeEntry {
                atom: AtomIdx::from_raw(key.raw()),
                flags: PropertyFlags::data(true, false, true),
            }],
            vec![PropertySlot::auto_init(initializer)].into(),
        )
        .unwrap();
}

fn own_slot(state: &RuntimeState, object: ObjectId, key: Atom) -> (&PropertySlot, PropertyFlags) {
    let object = state.heap.object(object).unwrap();
    let shape = state.heap.shape(object.shape).unwrap();
    let index = shape.find(AtomIdx::from_raw(key.raw())).unwrap() as usize;
    (&object.slots[index], shape.entries()[index].flags)
}

fn own_object(state: &RuntimeState, object: ObjectId, key: Atom) -> ObjectId {
    let (PropertySlot::Data(RawValue::Object(value)), _) = own_slot(state, object, key) else {
        panic!("materialized object result");
    };
    *value
}

fn queue_error() -> RuntimeError {
    RuntimeError::Heap(HeapError::Invariant(
        "finalization count disagrees with its queue/cycle state",
    ))
}

fn queue_invalid_zero(state: &mut RuntimeState, first: ObjectId, later: ObjectId) {
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

#[test]
fn all_eight_autoinit_families_complete_in_one_state_borrow() {
    let runtime = Runtime::new();
    let _caller = runtime.new_context().unwrap();
    let defining = runtime.new_context().unwrap();
    let objects: Vec<_> = (0..8).map(|_| runtime.new_object(None).unwrap()).collect();
    let keys: Vec<_> = (0..8)
        .map(|index| {
            runtime
                .intern_property_key(&format!("family{index}"))
                .unwrap()
        })
        .collect();
    let constructor = runtime.intern_property_key("constructor").unwrap();
    let realm = defining.realm;
    let initializers = [
        AutoInitProperty::FunctionPrototype { realm },
        AutoInitProperty::NativeBuiltin {
            realm,
            target: crate::engine::builtins::native::NativeFunctionId::MathClz32,
            name: "probe",
            length: 7,
            min_readable_args: 3,
        },
        AutoInitProperty::String {
            realm,
            value: "lazy string",
        },
        AutoInitProperty::ArrayUnscopables { realm },
        AutoInitProperty::Math { realm },
        AutoInitProperty::Reflect { realm },
        AutoInitProperty::Json { realm },
        AutoInitProperty::Atomics { realm },
    ];
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    let object_prototype = state.heap.context(realm).unwrap().object_prototype;
    let function_prototype = state.heap.context(realm).unwrap().function_prototype;
    for ((object, key), initializer) in objects.iter().zip(&keys).zip(initializers) {
        install_lazy(
            &mut state,
            &runtime.0.poisoned,
            object.object_id(),
            key.atom(),
            initializer,
        );
        let realm_before = state.heap.context_strong_count(realm).unwrap();
        let published = state
            .materialize_auto_init_property_with_publication(
                &runtime.0.poisoned,
                object.object_id(),
                key.atom(),
            )
            .unwrap();
        assert_eq!(
            published,
            !matches!(initializer, AutoInitProperty::String { .. })
        );
        assert_eq!(
            own_slot(&state, object.object_id(), key.atom()).1,
            PropertyFlags::data(true, false, true)
        );
        let realm_added = if matches!(initializer, AutoInitProperty::String { .. }) {
            let (PropertySlot::Data(RawValue::String(string)), _) =
                own_slot(&state, object.object_id(), key.atom())
            else {
                panic!("string result")
            };
            assert_eq!(
                state.heap.string(*string).unwrap(),
                &JsString::from_static("lazy string")
            );
            assert_eq!(state.heap.strong_count(RawId::String(*string)), Ok(1));
            0
        } else {
            let result = own_object(&state, object.object_id(), key.atom());
            assert_eq!(state.heap.object_strong_count(result), Ok(1));
            let data = state.heap.object(result).unwrap();
            let shape = state.heap.shape(data.shape).unwrap();
            if matches!(initializer, AutoInitProperty::FunctionPrototype { .. }) {
                assert_eq!(shape.prototype(), Some(object_prototype));
                assert_eq!(
                    own_object(&state, result, constructor.atom()),
                    object.object_id()
                );
                assert_eq!(state.heap.object_strong_count(object.object_id()), Ok(2));
                0
            } else if matches!(initializer, AutoInitProperty::NativeBuiltin { .. }) {
                assert_eq!(shape.prototype(), Some(function_prototype));
                let ObjectPayload::NativeFunction { data, .. } = &data.payload else {
                    panic!("native payload")
                };
                assert_eq!(
                    data.target,
                    crate::engine::builtins::native::NativeFunctionId::MathClz32
                );
                assert_eq!(data.realm, Some(realm));
                assert_eq!(data.min_readable_args, 3);
                let length = state
                    .pinned_atoms
                    .get(crate::engine::atom::pinned::PinnedAtom::Length);
                let name = state
                    .pinned_atoms
                    .get(crate::engine::atom::pinned::PinnedAtom::Name);
                assert!(
                    matches!(own_slot(&state, result, length), (PropertySlot::Data(RawValue::Int(7)), flags) if flags == PropertyFlags::data(false, false, true))
                );
                let (PropertySlot::Data(RawValue::String(name)), flags) =
                    own_slot(&state, result, name)
                else {
                    panic!("native name")
                };
                assert_eq!(flags, PropertyFlags::data(false, false, true));
                assert_eq!(
                    state.heap.string(*name).unwrap(),
                    &JsString::from_static("probe")
                );
                1
            } else if matches!(initializer, AutoInitProperty::ArrayUnscopables { .. }) {
                assert_eq!(data.kind, ObjectKind::Ordinary);
                assert_eq!(shape.prototype(), None);
                assert_eq!(shape.entries().len(), 16);
                assert!(
                    data.slots
                        .iter()
                        .all(|slot| matches!(slot, PropertySlot::Data(RawValue::Bool(true))))
                );
                assert!(
                    shape
                        .entries()
                        .iter()
                        .all(|entry| entry.flags == PropertyFlags::data(true, true, true))
                );
                0
            } else {
                assert_eq!(shape.prototype(), Some(object_prototype));
                let expected = match initializer {
                    AutoInitProperty::Math { .. } => "Math",
                    AutoInitProperty::Reflect { .. } => "Reflect",
                    AutoInitProperty::Json { .. } => "JSON",
                    AutoInitProperty::Atomics { .. } => "Atomics",
                    _ => unreachable!(),
                };
                let tag =
                    state.well_known_symbols[&crate::engine::object::WellKnownSymbol::ToStringTag];
                let (PropertySlot::Data(RawValue::String(string)), flags) =
                    own_slot(&state, result, tag)
                else {
                    panic!("intrinsic tag")
                };
                assert_eq!(
                    state.heap.string(*string).unwrap(),
                    &JsString::try_from_utf8(expected).unwrap()
                );
                assert_eq!(flags, PropertyFlags::data(false, false, true));
                data.slots
                    .iter()
                    .filter(|slot| matches!(slot, PropertySlot::AutoInit(_)))
                    .count() as u32
            }
        };
        assert_eq!(
            state.heap.context_strong_count(realm),
            Ok(realm_before - 1 + realm_added)
        );
        let counts = state.heap.counts();
        assert!(
            !state
                .materialize_auto_init_property_with_publication(
                    &runtime.0.poisoned,
                    object.object_id(),
                    key.atom()
                )
                .unwrap()
        );
        assert_eq!(state.heap.counts(), counts, "completion is idempotent");
    }
    assert!(!runtime.is_poisoned());
}

#[test]
fn function_prototype_preserves_checked_constructor_temporary_then_slot_retain() {
    for (count, succeeds) in [
        (u32::MAX - 3, true),
        (u32::MAX - 2, true),
        (u32::MAX - 1, false),
        (u32::MAX, false),
    ] {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let function = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("prototype").unwrap();
        let _unwind = runtime.unwind_guard();
        let mut state = runtime.0.state.borrow_mut();
        install_lazy(
            &mut state,
            &runtime.0.poisoned,
            function.object_id(),
            key.atom(),
            AutoInitProperty::FunctionPrototype {
                realm: context.realm,
            },
        );
        let realm_before = state.heap.context_strong_count(context.realm).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(function.object_id()), count);
        let result = state.materialize_auto_init_property(
            &runtime.0.poisoned,
            function.object_id(),
            key.atom(),
        );
        let observed_count = state
            .heap
            .object_strong_count(function.object_id())
            .unwrap();
        // MAX is the existing immortal sentinel, including checked retains
        // which reach it. Restore real owners before any assertion can unwind.
        state.heap.set_strong_count_for_test(
            RawId::Object(function.object_id()),
            if succeeds { 2 } else { 1 },
        );
        if succeeds {
            result.unwrap();
            assert_eq!(
                observed_count,
                if count == u32::MAX - 3 {
                    count + 1
                } else {
                    u32::MAX
                }
            );
            assert!(matches!(
                own_slot(&state, function.object_id(), key.atom()).0,
                PropertySlot::Data(RawValue::Object(_))
            ));
        } else {
            assert!(matches!(
                result,
                Err(RuntimeError::Heap(HeapError::Overflow { .. }))
            ));
            assert_eq!(observed_count, u32::MAX);
            assert!(matches!(
                own_slot(&state, function.object_id(), key.atom()).0,
                PropertySlot::Data(RawValue::Undefined)
            ));
        }
        assert_eq!(
            state.heap.context_strong_count(context.realm),
            Ok(realm_before - 1)
        );
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn function_prototype_base_retain_failure_is_terminal_and_recoverable() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let function = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("prototype").unwrap();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    install_lazy(
        &mut state,
        &runtime.0.poisoned,
        function.object_id(),
        key.atom(),
        AutoInitProperty::FunctionPrototype {
            realm: context.realm,
        },
    );
    let base = state.heap.context(context.realm).unwrap().object_prototype;
    let original = state.heap.object_strong_count(base).unwrap();
    let before = state.heap.counts();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(base), u32::MAX);
    let result =
        state.materialize_auto_init_property(&runtime.0.poisoned, function.object_id(), key.atom());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(base), original);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(matches!(
        own_slot(&state, function.object_id(), key.atom()).0,
        PropertySlot::Data(RawValue::Undefined)
    ));
    assert_eq!(state.heap.counts().object_nodes, before.object_nodes);
    assert!(!runtime.is_poisoned());
}

#[test]
fn fresh_object_field_consumes_original_producer_on_rejection_without_an_extra_duplicate() {
    let runtime = Runtime::new();
    let destination = runtime.new_object(None).unwrap();
    let value = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(value), u32::MAX - 1);
    state
        .define_fresh_function_object_property(
            &runtime.0.poisoned,
            destination.object_id(),
            "edge",
            value,
            true,
            true,
        )
        .unwrap();
    let observed_count = state.heap.object_strong_count(value).unwrap();
    // The slot retain reaches the existing immortal sentinel. Restore the
    // actual single stored edge before an assertion can unwind.
    state
        .heap
        .set_strong_count_for_test(RawId::Object(value), 1);
    assert_eq!(observed_count, u32::MAX);
    // Public allocation occurs after the state lease ends.
    drop(state);
    let rejected = runtime.new_object(None).unwrap();
    let orphan = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_object_extensible(rejected.object_id(), false)
        .unwrap();
    let result = state.define_fresh_function_object_property(
        &runtime.0.poisoned,
        rejected.object_id(),
        "rejected",
        orphan,
        true,
        true,
    );
    assert_eq!(
        result,
        Err(RuntimeError::Invariant(
            "function intrinsic property definition was rejected"
        ))
    );
    assert!(state.heap.object(orphan).is_err());
    assert!(!runtime.is_poisoned());
}

#[test]
fn string_autoinit_publication_failure_quarantines_producer_and_suffix() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("string").unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    install_lazy(
        &mut state,
        &runtime.0.poisoned,
        object.object_id(),
        key.atom(),
        AutoInitProperty::String {
            realm: context.realm,
            value: "published",
        },
    );
    let realm_before = state.heap.context_strong_count(context.realm).unwrap();
    queue_invalid_zero(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.materialize_auto_init_property(
                &runtime.0.poisoned,
                object.object_id(),
                key.atom()
            ),
            Err(queue_error())
        );
        assert!(runtime.is_poisoned());
    }
    let (PropertySlot::Data(RawValue::String(string)), flags) =
        own_slot(&state, object.object_id(), key.atom())
    else {
        panic!("published string")
    };
    assert_eq!(flags, PropertyFlags::data(true, false, true));
    assert_eq!(state.heap.strong_count(RawId::String(*string)), Ok(2));
    assert_eq!(
        state.heap.context_strong_count(context.realm),
        Ok(realm_before - 1)
    );
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn interrupted_function_factory_does_not_terminalize_after_publication_quarantine() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let base = runtime
        .0
        .state
        .borrow()
        .heap
        .context(context.realm)
        .unwrap()
        .object_prototype;
    let base_root = ObjectRef::from_borrowed_handle(runtime.clone(), base).unwrap();
    let _matching_layout = runtime.new_object(Some(&base_root)).unwrap();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("prototype").unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    install_lazy(
        &mut state,
        &runtime.0.poisoned,
        object.object_id(),
        key.atom(),
        AutoInitProperty::FunctionPrototype {
            realm: context.realm,
        },
    );
    let realm_before = state.heap.context_strong_count(context.realm).unwrap();
    let base_before = state.heap.object_strong_count(base).unwrap();
    let nodes_before = state.heap.counts().object_nodes;
    queue_invalid_zero(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.materialize_auto_init_property(
                &runtime.0.poisoned,
                object.object_id(),
                key.atom()
            ),
            Err(queue_error())
        );
        assert!(runtime.is_poisoned());
    }
    assert!(matches!(
        own_slot(&state, object.object_id(), key.atom()).0,
        PropertySlot::AutoInit(_)
    ));
    assert_eq!(
        state.heap.context_strong_count(context.realm),
        Ok(realm_before)
    );
    assert_eq!(state.heap.counts().object_nodes, nodes_before + 1);
    assert_eq!(state.heap.object_strong_count(base), Ok(base_before + 1));
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn terminal_autoinit_cleanup_failure_overrides_initializer_error_before_suffix_cleanup() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("failed").unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    install_lazy(
        &mut state,
        &runtime.0.poisoned,
        object.object_id(),
        key.atom(),
        AutoInitProperty::FailureProbe {
            realm: context.realm,
        },
    );
    let realm_before = state.heap.context_strong_count(context.realm).unwrap();
    queue_invalid_zero(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.materialize_auto_init_property(
                &runtime.0.poisoned,
                object.object_id(),
                key.atom()
            ),
            Err(queue_error())
        );
        assert!(runtime.is_poisoned());
    }
    assert!(
        matches!(own_slot(&state, object.object_id(), key.atom()), (PropertySlot::Data(RawValue::Undefined), flags) if flags == PropertyFlags::data(true, false, true))
    );
    assert_eq!(
        state.heap.context_strong_count(context.realm),
        Ok(realm_before - 1)
    );
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
}

#[test]
fn non_string_adapter_admits_pending_fifo_before_factory_checked_retain() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("prototype").unwrap();
    runtime
        .define_function_auto_init_prototype(&object, context.realm)
        .unwrap();
    let stale = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let (base, original, realm_before) = {
        let mut state = runtime.0.state.borrow_mut();
        state.release_jsvalue(JsValue::Object(stale)).unwrap();
        let base = state.heap.context(context.realm).unwrap().object_prototype;
        let original = state.heap.object_strong_count(base).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(base), u32::MAX);
        (
            base,
            original,
            state.heap.context_strong_count(context.realm).unwrap(),
        )
    };
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(stale));
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(later));
    // This is the approved admission contraction: FIFO failure wins over
    // the simultaneously failing prototype retain before initialization.
    let error = runtime
        .materialize_auto_init_property(&object, &key)
        .unwrap_err();
    assert!(
        matches!(error, RuntimeError::Heap(_))
            && !matches!(error, RuntimeError::Heap(HeapError::Overflow { .. }))
    );
    assert!(runtime.is_poisoned());
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(base), original);
    assert!(matches!(
        own_slot(&state, object.object_id(), key.atom()).0,
        PropertySlot::AutoInit(_)
    ));
    assert_eq!(
        state.heap.context_strong_count(context.realm),
        Ok(realm_before)
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(1));
    assert!(runtime.0.deferred_references.has_pending());
}

#[test]
fn string_adapter_preserves_no_operation_boundary() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("lazy").unwrap();
    runtime
        .define_string_auto_init(&object, context.realm, "lazy", "value")
        .unwrap();
    let pending = runtime.new_object(None).unwrap().into_handle();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(pending));
    runtime
        .materialize_auto_init_property(&object, &key)
        .unwrap();
    assert!(runtime.0.deferred_references.has_pending());
    {
        let state = runtime.0.state.borrow();
        assert!(matches!(
            own_slot(&state, object.object_id(), key.atom()).0,
            PropertySlot::Data(RawValue::String(_))
        ));
        assert_eq!(state.heap.object_strong_count(pending), Ok(1));
    }
    runtime.drain_deferred_references().unwrap();
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(runtime.0.state.borrow().heap.object(pending).is_err());
}

#[test]
fn fresh_object_field_publication_failure_preserves_consumed_producer_and_atom_owner() {
    let runtime = Runtime::new();
    let destination = runtime.new_object(None).unwrap();
    let _shared_empty_layout = runtime.new_object(None).unwrap();
    let value = runtime.new_object(None).unwrap().into_handle();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime.new_object(None).unwrap().into_handle();
    let _unwind = runtime.unwind_guard();
    let mut state = runtime.0.state.borrow_mut();
    queue_invalid_zero(&mut state, first, later);
    {
        let mut suffix_owner = OwnedValuesGuard::new(
            &mut state,
            &runtime.0.poisoned,
            vec![JsValue::Object(suffix)],
        );
        let (state, _) = suffix_owner.parts();
        assert_eq!(
            state.define_fresh_function_object_property(
                &runtime.0.poisoned,
                destination.object_id(),
                "published-field",
                value,
                true,
                true
            ),
            Err(queue_error())
        );
        assert!(runtime.is_poisoned());
    }
    let object = state.heap.object(destination.object_id()).unwrap();
    let shape = state.heap.shape(object.shape).unwrap();
    let atom = state.atoms.brand(shape.entries()[0].atom).unwrap();
    assert_eq!(
        state.atoms.resolve(atom).unwrap().ref_count,
        Some(2),
        "published key and quarantined intern producer"
    );
    assert_eq!(own_object(&state, destination.object_id(), atom), value);
    assert_eq!(
        state.heap.object_strong_count(value),
        Ok(2),
        "published slot and quarantined consumed producer"
    );
    assert_eq!(state.heap.object_strong_count(suffix), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
}
