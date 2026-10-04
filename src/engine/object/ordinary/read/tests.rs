use super::*;
use crate::engine::{
    api::context::Context,
    heap::{PropertySlot, RawId},
    object::OrdinaryRead,
};

fn object(context: &mut Context, source: &str) -> ObjectRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("object fixture")
    };
    object
}
fn ready(step: ReadStep) -> OwnedRead {
    let ReadStep::Ready(read) = step else {
        panic!("unexpected shared mutex boundary")
    };
    read
}

#[test]
fn borrowed_prototype_chain_needs_no_temporary_owner_or_runtime_clone() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(&mut context, "({__proto__:{__proto__:{x:{marker:7}}}})");
    let key = runtime.intern_property_key("x").unwrap();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    {
        let mut state = runtime.0.state.borrow_mut();
        let first = state
            .heap
            .shape(state.heap.object(object.object_id()).unwrap().shape)
            .unwrap()
            .prototype()
            .unwrap();
        let last = state
            .heap
            .shape(state.heap.object(first).unwrap().shape)
            .unwrap()
            .prototype()
            .unwrap();
        let first_count = state.heap.object_strong_count(first).unwrap();
        let last_count = state.heap.object_strong_count(last).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(first), u32::MAX);
        state
            .heap
            .set_strong_count_for_test(RawId::Object(last), u32::MAX);
        let read = ready(
            state
                .prepare_ordinary_read_in_state(
                    &runtime.0.poisoned,
                    runtime.domain_id(),
                    object.object_id(),
                    key.atom(),
                    &JsValue::Object(object.object_id()),
                    false,
                    None,
                )
                .unwrap(),
        );
        let OwnedRead::Complete(Some(JsValue::Object(result))) = &read else {
            panic!("data result")
        };
        assert_eq!(state.heap.object_strong_count(*result), Ok(2));
        assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
        read.retire(&mut state, &runtime.0.poisoned).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(first), first_count);
        state
            .heap
            .set_strong_count_for_test(RawId::Object(last), last_count);
    }
    #[cfg(feature = "profiling")]
    assert!(
        !profile
            .snapshot()
            .owned_execution_events
            .contains_key("core.runtime_clone")
    );
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn getter_output_retains_only_consumed_getter_and_receiver() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(
        &mut context,
        "Object.defineProperty({},'x',{get(){return 7},set(v){}})",
    );
    let key = runtime.intern_property_key("x").unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let shape = state
        .heap
        .shape(state.heap.object(object.object_id()).unwrap().shape)
        .unwrap();
    let index = shape
        .find(crate::engine::atom::AtomIdx::from_raw(key.atom().raw()))
        .unwrap() as usize;
    let PropertySlot::Accessor { get, set } =
        &state.heap.object(object.object_id()).unwrap().slots[index]
    else {
        panic!("accessor")
    };
    let getter = get.option().unwrap();
    let setter = set.option().unwrap();
    let getter_count = state.heap.object_strong_count(getter).unwrap();
    let receiver_count = state.heap.object_strong_count(object.object_id()).unwrap();
    let setter_count = state.heap.object_strong_count(setter).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(setter), u32::MAX);
    let read = ready(
        state
            .prepare_ordinary_read_in_state(
                &runtime.0.poisoned,
                runtime.domain_id(),
                object.object_id(),
                key.atom(),
                &JsValue::Object(object.object_id()),
                false,
                None,
            )
            .unwrap(),
    );
    assert!(matches!(&read, OwnedRead::Getter { function, .. } if *function == getter));
    assert_eq!(state.heap.object_strong_count(getter), Ok(getter_count + 1));
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(receiver_count + 1)
    );
    read.retire(&mut state, &runtime.0.poisoned).unwrap();
    assert_eq!(state.heap.object_strong_count(getter), Ok(getter_count));
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(receiver_count)
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(setter), setter_count);
    assert!(!runtime.is_poisoned());
}

#[test]
fn selected_getter_checked_overflow_precedes_receiver_promotion() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(&mut context, "({get x(){return 7}})");
    let key = runtime.intern_property_key("x").unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let OwnPropertySelection::Ready(ReadyOwnProperty::Stored(
        CompletePropertyDescriptor::Accessor {
            get: Some(RawValue::Object(getter)),
            ..
        },
    )) = state
        .select_own_property(&runtime.0.poisoned, object.object_id(), key.atom())
        .unwrap()
    else {
        panic!("getter")
    };
    let count = state.heap.object_strong_count(getter).unwrap();
    let receiver_count = state.heap.object_strong_count(object.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(getter), u32::MAX);
    let Err(error) = state.prepare_ordinary_read_in_state(
        &runtime.0.poisoned,
        runtime.domain_id(),
        object.object_id(),
        key.atom(),
        &JsValue::Object(object.object_id()),
        false,
        None,
    ) else {
        panic!("getter overflow")
    };
    assert!(error.to_string().contains("overflow"));
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(receiver_count)
    );
    assert!(!runtime.is_poisoned());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(getter), count);
}

#[test]
fn aliased_getter_and_receiver_keep_two_independent_result_edges() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(
        &mut context,
        "(()=>{let f=function(){return 7};Object.defineProperty(f,'x',{get:f});return f})()",
    );
    let key = runtime.intern_property_key("x").unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let count = state.heap.object_strong_count(object.object_id()).unwrap();
    let read = ready(
        state
            .prepare_ordinary_read_in_state(
                &runtime.0.poisoned,
                runtime.domain_id(),
                object.object_id(),
                key.atom(),
                &JsValue::Object(object.object_id()),
                false,
                None,
            )
            .unwrap(),
    );
    assert!(
        matches!(&read, OwnedRead::Getter { function, receiver: JsValue::Object(receiver) } if *function == object.object_id() && *receiver == *function)
    );
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(count + 2)
    );
    read.retire(&mut state, &runtime.0.poisoned).unwrap();
    assert_eq!(
        state.heap.object_strong_count(object.object_id()),
        Ok(count)
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn receiver_checked_failure_retires_selected_getter_and_can_retry() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(&mut context, "({get x(){return 7}})");
    let key = runtime.intern_property_key("x").unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let OwnPropertySelection::Ready(ReadyOwnProperty::Stored(
        CompletePropertyDescriptor::Accessor {
            get: Some(RawValue::Object(getter)),
            ..
        },
    )) = state
        .select_own_property(&runtime.0.poisoned, object.object_id(), key.atom())
        .unwrap()
    else {
        panic!("getter")
    };
    let receiver_count = state.heap.object_strong_count(object.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object.object_id()), u32::MAX);
    let result = state.prepare_ordinary_read_in_state(
        &runtime.0.poisoned,
        runtime.domain_id(),
        object.object_id(),
        key.atom(),
        &JsValue::Object(object.object_id()),
        false,
        None,
    );
    let Err(error) = result else {
        panic!("checked receiver retain must reject")
    };
    assert!(error.to_string().contains("overflow"));
    assert_eq!(state.heap.object_strong_count(getter), Ok(1));
    assert!(!runtime.is_poisoned());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object.object_id()), receiver_count);
    let read = ready(
        state
            .prepare_ordinary_read_in_state(
                &runtime.0.poisoned,
                runtime.domain_id(),
                object.object_id(),
                key.atom(),
                &JsValue::Object(object.object_id()),
                false,
                None,
            )
            .unwrap(),
    );
    read.retire(&mut state, &runtime.0.poisoned).unwrap();
}

#[test]
fn checked_failure_cleanup_quarantines_before_later_queue_or_owner() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(&mut context, "({get x(){return 7}})");
    let key = runtime.intern_property_key("x").unwrap();
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
    state
        .heap
        .set_strong_count_for_test(RawId::Object(object.object_id()), u32::MAX);
    let Err(error) = state.prepare_ordinary_read_in_state(
        &runtime.0.poisoned,
        runtime.domain_id(),
        object.object_id(),
        key.atom(),
        &JsValue::Object(object.object_id()),
        false,
        None,
    ) else {
        panic!("failure")
    };
    assert!(error.to_string().contains("finalization count disagrees"));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn typed_numeric_absence_is_terminal_but_named_absence_walks_prototype() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(
        &mut context,
        "(()=>{let a=new Uint8Array([7]);Object.setPrototypeOf(a,{'9':99,'-0':88,get x(){return this[0]}});return a})()",
    );
    for spelling in ["-0", "9", "NaN"] {
        let key = runtime.intern_property_key(spelling).unwrap();
        let mut state = runtime.0.state.borrow_mut();
        let read = ready(
            state
                .prepare_ordinary_read_in_state(
                    &runtime.0.poisoned,
                    runtime.domain_id(),
                    object.object_id(),
                    key.atom(),
                    &JsValue::Object(object.object_id()),
                    false,
                    None,
                )
                .unwrap(),
        );
        assert!(matches!(
            read,
            OwnedRead::Complete(Some(JsValue::Undefined))
        ));
    }
    let key = runtime.intern_property_key("x").unwrap();
    let read = runtime
        .prepare_ordinary_read_selected(&object, &key, &JsValue::Object(object.object_id()), None)
        .unwrap();
    let crate::engine::value::conversion::NativeConversion::Value(Some(JsValue::Int(7))) = runtime
        .finish_prepared_read_jsvalue(context.realm, &key, read)
        .unwrap()
    else {
        panic!("named inherited getter")
    };
}

#[test]
fn shared_word_owns_backing_then_materializes_one_bigint_after_lease_ends() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(
        &mut context,
        "(()=>{let a=new BigUint64Array(new SharedArrayBuffer(8));a[0]=18446744073709551615n;return a})()",
    );
    let key = runtime.intern_property_key("0").unwrap();
    let step = runtime
        .0
        .state
        .borrow_mut()
        .prepare_ordinary_read_in_state(
            &runtime.0.poisoned,
            runtime.domain_id(),
            object.object_id(),
            key.atom(),
            &JsValue::Object(object.object_id()),
            false,
            None,
        )
        .unwrap();
    let ReadStep::Shared(word) = step else {
        panic!("actual shared backing boundary")
    };
    let word = word.read().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let read = state.own_typed_read_word(word).unwrap();
    let OwnedRead::Complete(Some(JsValue::BigInt(id))) = &read else {
        panic!("long BigInt")
    };
    assert_eq!(state.heap.strong_count(RawId::BigInt(*id)), Ok(1));
    let id = *id;
    read.retire(&mut state, &runtime.0.poisoned).unwrap();
    assert!(state.heap.bigint(id).is_err());
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn value_read_preserves_unboxed_string_units_length_and_primitive_receiver() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let input = runtime
        .into_jsvalue(Value::String(
            crate::engine::value::JsString::try_from_utf16([0xd800, 120]).unwrap(),
        ))
        .unwrap();
    let zero = runtime.intern_property_key("0").unwrap();
    let length = runtime.intern_property_key("length").unwrap();
    let read = runtime
        .prepare_value_property_read_borrowed_jsvalue(context.realm, &input, &zero)
        .unwrap();
    let OrdinaryRead::Complete(Some(value)) = read else {
        panic!("unit")
    };
    let Value::String(unit) = runtime.root_and_release_jsvalue(value).unwrap() else {
        panic!("unit String")
    };
    assert_eq!(unit.utf16_units().collect::<Vec<_>>(), vec![0xd800]);
    let read = runtime
        .prepare_value_property_read_borrowed_jsvalue(context.realm, &input, &length)
        .unwrap();
    assert!(matches!(
        read,
        OrdinaryRead::Complete(Some(JsValue::Int(2)))
    ));
    runtime.release_jsvalue(input).unwrap();
    drop(context.eval("Object.defineProperty(Number.prototype,'x',{get(){'use strict';return typeof this}})").unwrap());
    let key = runtime.intern_property_key("x").unwrap();
    let read = runtime
        .prepare_value_property_read_borrowed_jsvalue(context.realm, &JsValue::Int(7), &key)
        .unwrap();
    let crate::engine::value::conversion::NativeConversion::Value(Some(value)) = runtime
        .finish_prepared_read_jsvalue(context.realm, &key, read)
        .unwrap()
    else {
        panic!("getter")
    };
    assert_eq!(
        runtime.root_and_release_jsvalue(value).unwrap(),
        Value::String(crate::engine::value::JsString::from_static("number"))
    );
}

#[test]
fn selected_proxy_stays_alive_after_prototype_replacement() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(
        &mut context,
        "globalThis.base=Object.create(new Proxy({},{get(t,k,r){return r.marker}}));base.marker=17;base",
    );
    let key = runtime.intern_property_key("x").unwrap();
    let step = runtime
        .0
        .state
        .borrow_mut()
        .prepare_ordinary_read_in_state(
            &runtime.0.poisoned,
            runtime.domain_id(),
            object.object_id(),
            key.atom(),
            &JsValue::Object(object.object_id()),
            false,
            None,
        )
        .unwrap();
    let owned = ready(step);
    let OwnedRead::Proxy { object: proxy, .. } = &owned else {
        panic!("selected proxy")
    };
    let proxy = *proxy;
    drop(context.eval("Object.setPrototypeOf(base,null)").unwrap());
    // Discard the optional old shape-prefix root; the selected read owner
    // alone must now keep the Proxy live through the later dispatch.
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(proxy),
        Ok(1)
    );
    let read = runtime.adopt_prepared_read(owned);
    let crate::engine::value::conversion::NativeConversion::Value(Some(JsValue::Int(17))) = runtime
        .finish_prepared_read_jsvalue(context.realm, &key, read)
        .unwrap()
    else {
        panic!("selected Proxy")
    };
    assert!(runtime.0.state.borrow().heap.object(proxy).is_err());
}

#[test]
fn lazy_own_property_and_dense_hole_use_same_state_walker() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let function = object(&mut context, "function f(a,b){};f");
    let length = runtime.intern_property_key("length").unwrap();
    let read = runtime
        .prepare_ordinary_read_selected(
            &function,
            &length,
            &JsValue::Object(function.object_id()),
            None,
        )
        .unwrap();
    assert!(matches!(
        read,
        OrdinaryRead::Complete(Some(JsValue::Int(2)))
    ));
    let array = object(
        &mut context,
        "(()=>{let a=[1,,3];Object.setPrototypeOf(a,{'1':9});return a})()",
    );
    let one = runtime.intern_property_key("1").unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let own = ready(
        state
            .prepare_ordinary_read_in_state(
                &runtime.0.poisoned,
                runtime.domain_id(),
                array.object_id(),
                one.atom(),
                &JsValue::Object(array.object_id()),
                true,
                None,
            )
            .unwrap(),
    );
    assert!(matches!(own, OwnedRead::Complete(None)));
    let inherited = ready(
        state
            .prepare_ordinary_read_in_state(
                &runtime.0.poisoned,
                runtime.domain_id(),
                array.object_id(),
                one.atom(),
                &JsValue::Object(array.object_id()),
                false,
                None,
            )
            .unwrap(),
    );
    assert!(matches!(
        inherited,
        OwnedRead::Complete(Some(JsValue::Int(9)))
    ));
}

#[test]
fn state_read_does_not_drain_runtime_coordinator_but_adapter_admits_once() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let object = object(&mut context, "({x:7})");
    let key = runtime.intern_property_key("x").unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    {
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(first), 0);
        runtime
            .0
            .deferred_references
            .push_back(crate::engine::heap::runtime::DeferredRefOp::Object(first));
        runtime
            .0
            .deferred_references
            .push_back(crate::engine::heap::runtime::DeferredRefOp::Object(later));
        let read = ready(
            state
                .prepare_ordinary_read_in_state(
                    &runtime.0.poisoned,
                    runtime.domain_id(),
                    object.object_id(),
                    key.atom(),
                    &JsValue::Object(object.object_id()),
                    false,
                    None,
                )
                .unwrap(),
        );
        assert!(matches!(read, OwnedRead::Complete(Some(JsValue::Int(7)))));
        assert!(!runtime.is_poisoned());
        assert!(runtime.0.deferred_references.has_pending());
    }
    assert!(
        runtime
            .prepare_ordinary_read_selected(
                &object,
                &key,
                &JsValue::Object(object.object_id()),
                None
            )
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(later),
        Ok(1)
    );
    assert!(runtime.0.deferred_references.has_pending());
}

#[test]
fn value_adapter_admission_precedes_foreign_key_and_nullish_errors() {
    for string in [false, true] {
        let runtime = Runtime::new();
        let context = runtime.new_context().unwrap();
        let foreign = Runtime::new();
        let key = foreign.intern_property_key("x").unwrap();
        let receiver = if string {
            runtime
                .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
                    "x",
                )))
                .unwrap()
        } else {
            JsValue::Null
        };
        let healthy =
            runtime.prepare_value_property_read_borrowed_jsvalue(context.realm, &receiver, &key);
        assert!(matches!(
            healthy,
            Err(RuntimeError::WrongRuntime("property key"))
        ));
        let first = runtime.new_object(None).unwrap().into_handle();
        let later = runtime.new_object(None).unwrap().into_handle();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(first), 0);
        runtime
            .0
            .deferred_references
            .push_back(crate::engine::heap::runtime::DeferredRefOp::Object(first));
        runtime
            .0
            .deferred_references
            .push_back(crate::engine::heap::runtime::DeferredRefOp::Object(later));
        let Err(error) =
            runtime.prepare_value_property_read_borrowed_jsvalue(context.realm, &receiver, &key)
        else {
            panic!("admission failure")
        };
        assert!(!matches!(
            error,
            RuntimeError::WrongRuntime(_) | RuntimeError::Engine(_)
        ));
        assert!(runtime.is_poisoned());
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(later),
            Ok(1)
        );
        assert!(runtime.0.deferred_references.has_pending());
        // The sole String owner is intentionally quarantined with the heap.
    }
}

#[test]
fn lazy_native_fact_comes_from_materialized_owned_callee() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let math = object(&mut context, "Math");
    let key = runtime.intern_property_key("random").unwrap();
    let mut fact = None;
    let read = runtime
        .prepare_ordinary_read_selected(
            &math,
            &key,
            &JsValue::Object(math.object_id()),
            Some(&mut fact),
        )
        .unwrap();
    let OrdinaryRead::Complete(Some(JsValue::Object(function))) = read else {
        panic!("owned native callee")
    };
    let data = fact
        .take()
        .unwrap()
        .into_parts_in_domain(runtime.domain_id(), function)
        .unwrap();
    assert_eq!(
        data.target,
        crate::engine::builtins::native::NativeFunctionId::MathRandom
    );
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(function),
        Ok(2)
    );
    runtime.release_jsvalue(JsValue::Object(function)).unwrap();
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(function),
        Ok(1)
    );
    assert!(!runtime.is_poisoned());
}
