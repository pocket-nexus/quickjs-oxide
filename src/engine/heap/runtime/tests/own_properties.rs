use super::*;
use crate::engine::heap::{ObjectId, RawId, Slots};
use crate::engine::object::own_properties::{OwnPropertySelection, ReadyOwnProperty};
use crate::engine::object::property::CompletePropertyDescriptor;
use crate::engine::object::{ObjectRef, OwnedCompletePropertyDescriptor};

mod admission;
mod typed;

#[test]
fn own_lazy_result_has_one_owned_edge_and_ready_promotions_remain_checked_at_max() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context.eval("(function(){})").unwrap() else {
        panic!("function fixture")
    };
    let key = runtime.intern_property_key("prototype").unwrap();
    let owned = runtime
        .get_own_property_owned(&function, &key)
        .unwrap()
        .unwrap();
    let RawValue::Object(prototype) = data_value(&owned) else {
        panic!("lazy prototype result")
    };
    let prototype = *prototype;
    // Stored function.prototype plus one durable owned descriptor. The old
    // internal public descriptor promotion/drop roundtrip has been removed.
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(prototype),
        Ok(2)
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), u32::MAX - 1);
    let public = runtime.get_own_property(&function, &key).unwrap();
    let public_max = runtime.0.state.borrow().heap.object_strong_count(prototype);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), 3);
    assert_eq!(public_max, Ok(u32::MAX));
    drop(public);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), u32::MAX - 1);
    let next = runtime.get_own_property_owned(&function, &key).unwrap();
    let owned_max = runtime.0.state.borrow().heap.object_strong_count(prototype);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), 3);
    assert_eq!(owned_max, Ok(u32::MAX));
    drop(next);
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(prototype),
        Ok(2)
    );
}

fn store(runtime: &Runtime, object: &ObjectRef, key: &PropertyKey, slot: PropertySlot) {
    let flags = if matches!(slot, PropertySlot::Accessor { .. }) {
        PropertyFlags::accessor(true, true)
    } else {
        PropertyFlags::data(true, true, true)
    };
    runtime
        .0
        .state
        .borrow_mut()
        .store_property_slot(object.object_id(), key.atom(), flags, slot)
        .unwrap();
}

fn data_value(descriptor: &OwnedCompletePropertyDescriptor) -> &RawValue {
    let CompletePropertyDescriptor::Data { value, .. } = descriptor.record() else {
        panic!("expected a data descriptor")
    };
    value
}

fn pending_invalid(runtime: &Runtime) -> ObjectId {
    let first = runtime.new_object(None).unwrap().into_handle();
    runtime.release_jsvalue(JsValue::Object(first)).unwrap();
    let later = runtime.new_object(None).unwrap().into_handle();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(first));
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(later));
    later
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

#[test]
fn own_state_record_survives_slot_mutation_gc_and_transfers_one_edge() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let child = runtime.new_object(None).unwrap();
    let child_id = child.object_id();
    let key = runtime.intern_property_key("held").unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::Data(RawValue::Object(child_id)),
    );
    let record = {
        let mut state = runtime.0.state.borrow_mut();
        let OwnPropertySelection::Ready(ready) = state
            .select_own_property(&runtime.0.poisoned, object.object_id(), key.atom())
            .unwrap()
        else {
            panic!("stored data was not ready")
        };
        let record = state
            .own_selected_property_descriptor(&runtime.0.poisoned, ready)
            .unwrap();
        assert_eq!(state.heap.object_strong_count(child_id), Ok(3));
        record
    };
    let owned = OwnedCompletePropertyDescriptor::from_owned_record(&runtime, record);
    drop(child);
    runtime
        .0
        .state
        .borrow_mut()
        .replace_property_slot(object.object_id(), 0, PropertySlot::Data(RawValue::Int(8)))
        .unwrap();
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(child_id),
        Ok(1)
    );
    let value = owned.into_data_value().unwrap();
    assert!(matches!(value, JsValue::Object(id) if id == child_id));
    runtime.release_jsvalue(value).unwrap();
    assert!(runtime.0.state.borrow().heap.object(child_id).is_err());
}

#[test]
fn own_public_string_bigint_clone_payloads_at_max_owned_retains_overflow() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    for value in [
        Value::String(JsString::from_static("payload")),
        Value::BigInt(JsBigInt::from(u64::MAX)),
    ] {
        let key = runtime.intern_property_key("payload").unwrap();
        runtime
            .define_own_property(&object, &key, &data_descriptor(value, true, true, true))
            .unwrap();
        let target = {
            let mut state = runtime.0.state.borrow_mut();
            let OwnPropertySelection::Ready(ReadyOwnProperty::Stored(
                CompletePropertyDescriptor::Data { value, .. },
            )) = state
                .select_own_property(&runtime.0.poisoned, object.object_id(), key.atom())
                .unwrap()
            else {
                panic!("payload missing")
            };
            match value {
                RawValue::String(id) => RawId::String(id),
                RawValue::BigInt(id) => RawId::BigInt(id),
                _ => panic!("payload did not allocate an arena leaf"),
            }
        };
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(target, u32::MAX);
        let public = runtime.get_own_property(&object, &key);
        let owned = runtime.get_own_property_owned(&object, &key);
        let maximum = runtime.0.state.borrow().heap.strong_count(target);
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(target, 1);
        assert!(public.unwrap().is_some());
        assert!(matches!(
            owned,
            Err(RuntimeError::Heap(HeapError::Overflow { .. }))
        ));
        assert_eq!(maximum, Ok(u32::MAX));
        assert!(!runtime.is_poisoned());
    }
}

#[test]
fn own_public_object_and_accessor_use_only_existing_checked_promotions() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let edge = runtime.new_object(None).unwrap();
    let id = edge.object_id();
    let data = runtime.intern_property_key("data").unwrap();
    store(
        &runtime,
        &object,
        &data,
        PropertySlot::Data(RawValue::Object(id)),
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), u32::MAX - 1);
    let public = runtime.get_own_property(&object, &data).unwrap();
    let count = runtime.0.state.borrow().heap.object_strong_count(id);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), 3);
    assert_eq!(count, Ok(u32::MAX));
    drop(public);
    let accessor = runtime.intern_property_key("accessor").unwrap();
    store(
        &runtime,
        &object,
        &accessor,
        PropertySlot::accessor(Some(id), Some(id)),
    );
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), u32::MAX - 2);
    let public = runtime.get_own_property(&object, &accessor).unwrap();
    let count = runtime.0.state.borrow().heap.object_strong_count(id);
    // Root + data slot + two accessor slots + two public accessor roots.
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), 6);
    assert_eq!(count, Ok(u32::MAX));
    drop(public);
    assert_eq!(runtime.0.state.borrow().heap.object_strong_count(id), Ok(4));
}

#[test]
fn own_public_symbol_promotes_one_checked_atom_edge() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let symbol = runtime
        .new_symbol(Some(JsString::from_static("edge")))
        .unwrap();
    let atom = symbol.atom();
    let key = runtime.intern_property_key("symbol").unwrap();
    let index = runtime.0.state.borrow().atoms.unbrand(atom).unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::Data(RawValue::Symbol(index)),
    );
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(index, u32::MAX - 1);
    let public = runtime.get_own_property(&object, &key).unwrap();
    let count = runtime
        .0
        .state
        .borrow()
        .atoms
        .resolve(atom)
        .unwrap()
        .ref_count;
    runtime
        .0
        .state
        .borrow()
        .atoms
        .set_ref_count_for_test(index, 3);
    assert_eq!(count, Some(u32::MAX));
    drop(public);
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .atoms
            .resolve(atom)
            .unwrap()
            .ref_count,
        Some(2)
    );
}

#[test]
fn own_accessor_aliases_keep_two_edges_and_partial_retain_is_recoverable() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let getter = runtime.new_object(None).unwrap();
    let setter = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("accessor").unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::accessor(Some(getter.object_id()), Some(getter.object_id())),
    );
    let owned = runtime
        .get_own_property_owned(&object, &key)
        .unwrap()
        .unwrap();
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(getter.object_id()),
        Ok(5)
    );
    drop(owned);
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(getter.object_id()),
        Ok(3)
    );
    runtime
        .0
        .state
        .borrow_mut()
        .replace_property_slot(
            object.object_id(),
            0,
            PropertySlot::accessor(Some(getter.object_id()), Some(setter.object_id())),
        )
        .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
    let result = runtime.get_own_property_owned(&object, &key);
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
        .set_strong_count_for_test(RawId::Object(setter.object_id()), 2);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(getter_count, Ok(2));
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn own_partial_getter_cleanup_error_overrides_retain_and_stops_suffix() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let getter = runtime.new_object(None).unwrap();
    let setter = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("accessor").unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::accessor(Some(getter.object_id()), Some(setter.object_id())),
    );
    let (first, later) = invalid_zero_queue(&runtime);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), u32::MAX);
    let result = runtime.get_own_property_owned(&object, &key);
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(setter.object_id()), 2);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Invariant(
            "finalization count disagrees with its queue/cycle state",
        )))
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(getter.object_id()), Ok(2));
    assert_eq!(state.heap.object_strong_count(first), Ok(1));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn own_var_ref_live_aliases_tdz_and_private_metadata_keep_descriptor_contract() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("cell").unwrap();
    let alias = runtime.intern_property_key("alias").unwrap();
    let cell = runtime
        .new_var_ref(JsValue::Int(3), true, true, ClosureVariableKind::Normal)
        .unwrap();
    store(&runtime, &object, &key, PropertySlot::VarRef(cell.id()));
    store(&runtime, &object, &alias, PropertySlot::VarRef(cell.id()));
    let owned = runtime
        .get_own_property_owned(&object, &key)
        .unwrap()
        .unwrap();
    assert!(matches!(data_value(&owned), RawValue::Int(3)));
    runtime
        .0
        .state
        .borrow_mut()
        .write_var_ref(&runtime.0.poisoned, cell.id(), JsValue::Int(9))
        .unwrap();
    let next = runtime
        .get_own_property_owned(&object, &alias)
        .unwrap()
        .unwrap();
    assert!(matches!(data_value(&next), RawValue::Int(9)));
    assert!(matches!(data_value(&owned), RawValue::Int(3)));
    runtime
        .0
        .state
        .borrow_mut()
        .reset_var_ref_uninitialized(&runtime.0.poisoned, cell.id())
        .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .set_var_ref_metadata(cell.id(), true, true, ClosureVariableKind::PrivateField)
        .unwrap();
    assert!(matches!(
        runtime.0.state.borrow_mut().read_var_ref(cell.id()),
        Err(RuntimeError::Invariant(
            "ordinary VarRef read reached a private-element binding"
        ))
    ));
    let error = match runtime.get_own_property_owned(&object, &key) {
        Err(RuntimeError::Engine(error)) => error,
        _ => panic!("TDZ did not report its key error"),
    };
    assert_eq!(error.kind(), ErrorKind::Reference);
    assert_eq!(error.message(), "cell is not initialized");
    assert!(!runtime.is_poisoned());
}

#[test]
fn own_malformed_parallel_slot_is_checked_error_without_destructive_poison() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("slot").unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::Data(RawValue::Int(3)),
    );
    let slots = {
        let mut state = runtime.0.state.borrow_mut();
        std::mem::replace(
            &mut state.heap.object_mut(object.object_id()).unwrap().slots,
            Slots::from_vec(vec![]),
        )
    };
    let owned = runtime.get_own_property_owned(&object, &key);
    let public = runtime.get_own_property(&object, &key);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .object_mut(object.object_id())
        .unwrap()
        .slots = slots;
    assert!(matches!(
        owned,
        Err(RuntimeError::Invariant(
            "ordinary shape has no parallel slot"
        ))
    ));
    assert!(matches!(
        public,
        Err(RuntimeError::Invariant(
            "ordinary shape has no parallel slot"
        ))
    ));
    assert!(!runtime.is_poisoned());
}

#[test]
fn own_private_and_internal_sentinel_keep_representation_specific_errors() {
    let runtime = Runtime::new();
    let object = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("slot").unwrap();
    store(
        &runtime,
        &object,
        &key,
        PropertySlot::Data(RawValue::Int(3)),
    );
    let private = runtime
        .0
        .state
        .borrow_mut()
        .atoms
        .new_private_symbol(Some("hidden"))
        .unwrap();
    for (raw, message) in [
        (
            RawValue::Private(AtomIdx::from_raw(private.raw())),
            "private-name identity escaped into an ECMAScript Value",
        ),
        (
            RawValue::Exception,
            "internal value sentinel escaped from an object property",
        ),
    ] {
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .object_mut(object.object_id())
            .unwrap()
            .slots[0] = PropertySlot::Data(raw);
        let public = runtime.get_own_property(&object, &key);
        let owned = runtime.get_own_property_owned(&object, &key);
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .object_mut(object.object_id())
            .unwrap()
            .slots[0] = PropertySlot::Data(RawValue::Int(3));
        assert!(matches!(public, Err(RuntimeError::Invariant(error)) if error == message));
        assert!(matches!(
            owned,
            Err(RuntimeError::Invariant("descriptor held internal sentinel"))
        ));
        assert!(!runtime.is_poisoned());
    }
    runtime.0.state.borrow_mut().atoms.release(private).unwrap();
}

#[test]
fn own_set_proxy_integrity_consumers_keep_owned_edges_across_callbacks() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        context
            .eval(
                r#"(function(){
      var value={tag:7}, backing={x:value}, order=[];
      var target=new Proxy(backing,{isExtensible(t){order.push('extensible');return true}});
      var p=new Proxy(target,{getOwnPropertyDescriptor(t,k){
        return {get value(){order.push('value');delete backing.x;return value},
                configurable:true,enumerable:true,writable:true};
      }});
      var d=Object.getOwnPropertyDescriptor(p,'x');
      var receiver={},base={};
      Object.defineProperty(base,'x',{get(){return 1},set(v){
        delete base.x;Object.defineProperty(receiver,'kept',{value:v});
      },configurable:true});
      var set=Reflect.set(base,'x',value,receiver);
      Object.freeze(receiver);
      return d.value===value&&order.join(',')==='extensible,value'&&set&&
        receiver.kept===value&&Object.isFrozen(receiver);
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
}
