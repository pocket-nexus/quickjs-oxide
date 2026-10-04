//! Definition ownership, admission and destructive publication witnesses.

use super::*;
use crate::engine::heap::runtime::RuntimeState;
use crate::engine::heap::{ObjectId, RawId};
use crate::engine::object::property::PropertyDescriptor;
use crate::engine::object::{ObjectRef, OwnedPropertyDescriptor};

mod admission;
mod arguments;
mod arrays;
mod typed;

fn object(context: &mut crate::engine::api::context::Context, source: &str) -> ObjectRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("definition fixture did not return an object")
    };
    object
}

fn define_raw(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
    descriptor: &PropertyDescriptor<RawValue>,
) -> Result<bool, RuntimeError> {
    let _unwind = runtime.unwind_guard();
    runtime.0.state.borrow_mut().define_own_raw_property(
        &runtime.0.poisoned,
        object.object_id(),
        key.atom(),
        descriptor,
    )
}

fn store(
    runtime: &Runtime,
    object: &ObjectRef,
    key: &PropertyKey,
    flags: PropertyFlags,
    slot: PropertySlot,
) {
    let _unwind = runtime.unwind_guard();
    runtime
        .store_property_slot(object, key, flags, slot)
        .unwrap();
}

fn slot(state: &RuntimeState, object: ObjectId, key: &PropertyKey) -> PropertySlot {
    let data = state.heap.object(object).unwrap();
    let shape = state.heap.shape(data.shape).unwrap();
    data.slots[shape.find(AtomIdx::from_raw(key.atom().raw())).unwrap() as usize].clone()
}

fn dense_value(state: &RuntimeState, object: ObjectId, index: usize) -> Option<RawValue> {
    let ObjectPayload::Array { dense: Some(dense) } = &state.heap.object(object).unwrap().payload
    else {
        panic!("expected a dense Array")
    };
    dense.get(index).cloned()
}

fn queue_error() -> RuntimeError {
    RuntimeError::Heap(HeapError::Invariant(
        "finalization count disagrees with its queue/cycle state",
    ))
}

fn invalid_zero_queue(runtime: &Runtime) -> (ObjectId, ObjectId) {
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
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
    (first, later)
}

fn assert_stopped_zero_suffix(runtime: &Runtime, first: ObjectId, later: ObjectId) {
    assert!(runtime.is_poisoned());
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.object_strong_count(first), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert_eq!(state.heap.zero_queue.front(), Some(&RawId::Object(later)));
}

#[test]
fn definition_borrowed_current_does_not_promote_but_needed_store_remains_checked() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let child = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("held").unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::Data(RawValue::Object(child.object_id())),
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), u32::MAX);
    let result = define_raw(
        &runtime,
        &receiver,
        &key,
        &PropertyDescriptor {
            value: Some(RawValue::Int(8)),
            ..PropertyDescriptor::new()
        },
    );
    let count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(child.object_id());
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), 1);
    assert_eq!(result, Ok(true));
    // Heap release preserves its existing immortal MAX sentinel. The
    // replacement succeeds without requesting a checked current owner.
    assert_eq!(count, Ok(u32::MAX));
    assert!(!runtime.is_poisoned());
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), u32::MAX);
    let result = define_raw(
        &runtime,
        &receiver,
        &key,
        &PropertyDescriptor {
            value: Some(RawValue::Object(child.object_id())),
            ..PropertyDescriptor::new()
        },
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(matches!(
        slot(&runtime.0.state.borrow(), receiver.object_id(), &key),
        PropertySlot::Data(RawValue::Int(8))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_retain_failure_keeps_previous_slot_and_checked_partial_accessor_order() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let getter = runtime.new_object(None).unwrap();
    let setter = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("accessor").unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::Data(RawValue::Int(1)),
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
    let result = define_raw(
        &runtime,
        &receiver,
        &key,
        &PropertyDescriptor {
            get: Some(Some(RawValue::Object(getter.object_id()))),
            set: Some(Some(RawValue::Object(setter.object_id()))),
            ..PropertyDescriptor::new()
        },
    );
    let getter_count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(getter.object_id());
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(getter_count, Ok(1));
    assert!(matches!(
        slot(&runtime.0.state.borrow(), receiver.object_id(), &key),
        PropertySlot::Data(RawValue::Int(1))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_uninitialized_cell_precedes_value_production_and_preserves_metadata() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("lexical").unwrap();
    let cell = runtime
        .new_uninitialized_captured_var_ref(true, true, ClosureVariableKind::Normal)
        .unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::VarRef(cell.id()),
    );
    let before = runtime.0.state.borrow().heap.counts().string_nodes;
    let result = runtime.define_ordinary_own_property(
        &receiver,
        &key,
        &data_descriptor(
            Value::String(JsString::from_static("never produced")),
            true,
            true,
            true,
        ),
    );
    assert!(
        matches!(result, Err(RuntimeError::Engine(ref error)) if error.kind() == ErrorKind::Reference)
    );
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.counts().string_nodes, before);
    let metadata = state.heap.var_ref(cell.id()).unwrap();
    assert!(metadata.is_lexical && metadata.is_const);
    assert!(matches!(metadata.value, RawValue::Uninitialized));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_frozen_lazy_precheck_does_not_call_factory_or_produce_public_string() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("lazy").unwrap();
    let context = runtime.new_context().unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(false, false, false),
        PropertySlot::auto_init(crate::engine::heap::AutoInitProperty::String {
            realm: context.realm,
            value: "lazy",
        }),
    );
    let before = runtime.0.state.borrow().heap.counts().string_nodes;
    assert_eq!(
        runtime.define_ordinary_own_property(
            &receiver,
            &key,
            &data_descriptor(
                Value::String(JsString::from_static("never produced")),
                true,
                false,
                false
            )
        ),
        Ok(false)
    );
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.counts().string_nodes, before);
    assert!(matches!(
        slot(&state, receiver.object_id(), &key),
        PropertySlot::AutoInit(_)
    ));
}

#[test]
fn definition_lazy_factory_and_raw_current_validation_share_one_state_lease() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("lazy").unwrap();
    let context = runtime.new_context().unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, false, true),
        PropertySlot::auto_init(crate::engine::heap::AutoInitProperty::String {
            realm: context.realm,
            value: "old",
        }),
    );
    assert_eq!(
        define_raw(
            &runtime,
            &receiver,
            &key,
            &PropertyDescriptor {
                value: Some(RawValue::Int(11)),
                ..PropertyDescriptor::new()
            }
        ),
        Ok(true)
    );
    assert!(matches!(
        slot(&runtime.0.state.borrow(), receiver.object_id(), &key),
        PropertySlot::Data(RawValue::Int(11))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_string_virtual_compatibility_has_no_current_arena_producer() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let receiver = object(&mut context, "new String('x')");
    let key = runtime.property_key_for_index(0).unwrap();
    let before = runtime.0.state.borrow().heap.counts().string_nodes;
    assert_eq!(
        runtime.define_own_property(
            &receiver,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::String(JsString::from_static("x"))),
                ..OrdinaryPropertyDescriptor::new()
            }
        ),
        Ok(true)
    );
    assert_eq!(
        runtime.define_own_property(
            &receiver,
            &key,
            &OrdinaryPropertyDescriptor {
                value: DescriptorField::Present(Value::String(JsString::from_static("y"))),
                ..OrdinaryPropertyDescriptor::new()
            }
        ),
        Ok(false)
    );
    assert_eq!(runtime.0.state.borrow().heap.counts().string_nodes, before);
    let value = runtime
        .into_jsvalue(Value::String(JsString::from_static("x")))
        .unwrap();
    let mut descriptor = OwnedPropertyDescriptor::new(&runtime);
    descriptor.value = DescriptorField::Present(value);
    assert_eq!(
        runtime.define_owned_property_after_conversion_selection(&receiver, &key, &descriptor),
        Ok(true)
    );
    assert_eq!(
        runtime.0.state.borrow().heap.counts().string_nodes,
        before + 1
    );
}

#[test]
fn definition_malformed_parallel_slot_returns_checked_invariant_without_quarantine() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("missing slot").unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::Data(RawValue::Int(1)),
    );
    let slots = std::mem::take(
        &mut runtime
            .0
            .state
            .borrow_mut()
            .heap
            .object_mut(receiver.object_id())
            .unwrap()
            .slots,
    );
    let result = define_raw(
        &runtime,
        &receiver,
        &key,
        &PropertyDescriptor {
            value: Some(RawValue::Int(2)),
            ..PropertyDescriptor::new()
        },
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .object_mut(receiver.object_id())
        .unwrap()
        .slots = slots;
    assert_eq!(
        result,
        Err(RuntimeError::Invariant(
            "ordinary shape has no parallel slot"
        ))
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_published_slot_cleanup_failure_stops_suffix_and_keeps_published_value() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let child = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("held").unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::Data(RawValue::Object(child.object_id())),
    );
    let (first, later) = invalid_zero_queue(&runtime);
    assert_eq!(
        define_raw(
            &runtime,
            &receiver,
            &key,
            &PropertyDescriptor {
                value: Some(RawValue::Int(2)),
                ..PropertyDescriptor::new()
            }
        ),
        Err(queue_error())
    );
    assert_stopped_zero_suffix(&runtime, first, later);
    assert!(matches!(
        slot(&runtime.0.state.borrow(), receiver.object_id(), &key),
        PropertySlot::Data(RawValue::Int(2))
    ));
}

#[test]
fn definition_namespace_owned_compatibility_borrows_current_public_promotion_stays_checked() {
    let runtime = Runtime::new();
    let namespace = runtime.new_module_namespace_object().unwrap();
    let child = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("exported").unwrap();
    let cell = runtime
        .new_var_ref(
            runtime
                .dup_jsvalue(&JsValue::Object(child.object_id()))
                .unwrap(),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
    store(
        &runtime,
        &namespace,
        &key,
        PropertyFlags::data(true, true, false),
        PropertySlot::VarRef(cell.id()),
    );
    let descriptor = OwnedPropertyDescriptor::new(&runtime);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), u32::MAX);
    let owned =
        runtime.define_owned_property_after_conversion_selection(&namespace, &key, &descriptor);
    let public = runtime.define_own_property(&namespace, &key, &OrdinaryPropertyDescriptor::new());
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), 2);
    assert_eq!(owned, Ok(true));
    assert!(matches!(
        public,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_namespace_attribute_rejection_still_reads_uninitialized_binding_first() {
    let runtime = Runtime::new();
    let namespace = runtime.new_module_namespace_object().unwrap();
    let key = runtime.intern_property_key("exported").unwrap();
    let cell = runtime
        .new_uninitialized_captured_var_ref(true, false, ClosureVariableKind::Normal)
        .unwrap();
    store(
        &runtime,
        &namespace,
        &key,
        PropertyFlags::data(true, true, false),
        PropertySlot::VarRef(cell.id()),
    );
    let result = define_raw(
        &runtime,
        &namespace,
        &key,
        &PropertyDescriptor {
            configurable: Some(true),
            ..PropertyDescriptor::new()
        },
    );
    assert!(
        matches!(result, Err(RuntimeError::Engine(ref error)) if error.kind() == ErrorKind::Reference)
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn definition_public_string_bigint_producers_retire_after_storage_retains_one_edge() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("payload").unwrap();
    for value in [
        Value::String(JsString::from_static("heap string")),
        Value::BigInt(JsBigInt::from(u64::MAX)),
    ] {
        assert_eq!(
            runtime.define_own_property(&receiver, &key, &data_descriptor(value, true, true, true)),
            Ok(true)
        );
        let state = runtime.0.state.borrow();
        let target = match slot(&state, receiver.object_id(), &key) {
            PropertySlot::Data(RawValue::String(id)) => RawId::String(id),
            PropertySlot::Data(RawValue::BigInt(id)) => RawId::BigInt(id),
            _ => panic!("public producer did not store its heap payload"),
        };
        assert_eq!(state.heap.strong_count(target), Ok(1));
        assert!(!runtime.0.deferred_references.has_pending());
    }
}

#[test]
fn definition_incoming_symbol_atom_retain_overflow_is_checked_before_publication() {
    let runtime = Runtime::new();
    let receiver = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("symbol").unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("edge")))
        .unwrap();
    let index = runtime
        .0
        .state
        .borrow()
        .atoms
        .unbrand(symbol.atom())
        .unwrap();
    store(
        &runtime,
        &receiver,
        &key,
        PropertyFlags::data(true, true, true),
        PropertySlot::Data(RawValue::Int(1)),
    );
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(index, u32::MAX);
    let result = define_raw(
        &runtime,
        &receiver,
        &key,
        &PropertyDescriptor {
            value: Some(RawValue::Symbol(index)),
            ..PropertyDescriptor::new()
        },
    );
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(index, 1);
    assert!(matches!(
        result,
        Err(RuntimeError::Atom(
            crate::engine::atom::AtomError::RefCountOverflow(_)
        ))
    ));
    assert!(!runtime.is_poisoned());
    assert!(matches!(
        slot(&runtime.0.state.borrow(), receiver.object_id(), &key),
        PropertySlot::Data(RawValue::Int(1))
    ));
}
