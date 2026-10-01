use super::*;
use crate::engine::{
    api::{Context, GcPolicy},
    heap::RawId,
    vm::{call::ordinary::OrdinaryCall, execution::ExecutionLimits},
};

fn fixture(
    runtime: &Runtime,
    defining: &mut Context,
    caller: &Context,
    frames: usize,
) -> (RunningExecution, FrameId) {
    let Value::Object(function) = defining.eval("(function(){return 42})").unwrap() else {
        panic!("bytecode function")
    };
    let call = OrdinaryCall::select_callback(runtime, &function)
        .unwrap()
        .unwrap();
    let mut execution =
        RunningExecution::new(runtime, ExecutionLimits { frames, slots: 32 }).unwrap();
    let entry = call
        .prepare_callback(
            &mut execution.call_storage,
            JsValue::Undefined,
            Vec::new(),
            caller.realm,
            ReturnTarget {
                owner: ReturnOwner::Root,
                value_use: ReturnValue::Push,
                tail: false,
                operation: None,
            },
        )
        .unwrap();
    let id = push_frame(&mut execution, entry).unwrap();
    execution.frames.materialize(runtime).unwrap();
    (execution, id)
}

#[test]
fn public_field_local_completion_preserves_alias_owners_pc_and_identity() {
    for cached in [false, true] {
        let runtime = Runtime::new();
        runtime.set_gc_policy(GcPolicy::Manual);
        let mut context = runtime.new_context();
        let caller = runtime.new_context();
        let (mut execution, id) = fixture(&runtime, &mut context, &caller, 1);
        if cached {
            let query =
                execution
                    .query_storage
                    .acquire(context.realm, Vec::new(), Finish::Discard(0));
            query.recycle(&mut execution.query_storage);
        }
        let target = runtime.new_object(None).unwrap();
        let key = runtime.intern_property_key("self").unwrap();
        let next_pc = execution.frames.current_mut(id).unwrap().next_pc().unwrap();
        assert!(!execution.frames.can_push_with_continuations(0));
        #[cfg(feature = "profiling")]
        let profile = crate::engine::api::profiling::CostProfile::start();
        let result = start_public_field(
            &runtime,
            &mut execution,
            id,
            target.clone(),
            key.clone(),
            JsValue::Object(target.clone().into_handle()),
            0,
        )
        .unwrap();
        assert!(matches!(result, CallStep::Entered));
        let frame = execution.frames.current_mut(id).unwrap();
        assert_eq!(frame.property_generation, 1);
        assert_eq!(frame.resume_pc, next_pc);
        assert_eq!(frame.fault_pc, 0);
        assert_eq!(execution.slots.depth(&frame.window), 0);
        assert!(!frame.has_pending_query());
        assert!(frame.active_frame.is_materialized());
        assert_eq!(execution.query_storage.has_cached_entry(), cached);
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(target.object_id()),
            Ok(2)
        );
        #[cfg(feature = "profiling")]
        {
            let events = profile.snapshot().owned_execution_events;
            assert_eq!(events.get("public_field_completed_without_query"), Some(&1));
            assert!(!events.contains_key("query_dispatch"));
        }
        let descriptor = runtime.get_own_property(&target, &key).unwrap().unwrap();
        assert!(matches!(
            &descriptor,
            crate::engine::object::CompleteOrdinaryPropertyDescriptor::Data {
                value: Value::Object(value), writable: true, enumerable: true, configurable: true,
            } if value == &target
        ));
        drop(descriptor);
        drop(execution);
        assert!(runtime.0.state.borrow().active_frames.is_empty());
    }
}

#[test]
fn public_field_identity_exhaustion_precedes_definition_and_releases_inputs() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let caller = runtime.new_context();
    let (mut execution, id) = fixture(&runtime, &mut context, &caller, 1);
    execution
        .frames
        .current_mut(id)
        .unwrap()
        .property_generation = u64::MAX;
    let target = runtime.new_object(None).unwrap();
    let key = runtime.intern_property_key("self").unwrap();
    let result = start_public_field(
        &runtime,
        &mut execution,
        id,
        target.clone(),
        key.clone(),
        JsValue::Object(target.clone().into_handle()),
        0,
    );
    let Err(error) = result else {
        panic!("expected identity exhaustion")
    };
    assert!(error.message().contains("identity exhausted"));
    assert!(runtime.get_own_property(&target, &key).unwrap().is_none());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(target.object_id()),
        Ok(1)
    );
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(frame.property_generation, u64::MAX);
    assert_eq!(frame.resume_pc, 0);
    assert!(!frame.has_pending_query());
    assert!(!execution.query_storage.has_cached_entry());
    drop(execution);
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn public_field_proxy_budget_still_precedes_getter_trap_and_mutation() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let caller = runtime.new_context();
    let Value::Object(proxy) = context
        .eval("globalThis.fieldCalls=0;globalThis.fieldTarget={};new Proxy(fieldTarget,{get defineProperty(){fieldCalls++;return function(){fieldCalls++;return true}}})")
        .unwrap()
    else {
        panic!("proxy")
    };
    let (mut execution, id) = fixture(&runtime, &mut context, &caller, 1);
    let marker = runtime.new_object(None).unwrap();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let result = start_public_field(
        &runtime,
        &mut execution,
        id,
        proxy,
        runtime.intern_property_key("x").unwrap(),
        JsValue::Object(marker.clone().into_handle()),
        0,
    )
    .unwrap();
    let CallStep::Complete(Completion::Throw(thrown)) = result else {
        panic!("expected budget rejection")
    };
    runtime.release_jsvalue(thrown).unwrap();
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(marker.object_id()),
        Ok(1)
    );
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(frame.property_generation, 1);
    assert_eq!(frame.resume_pc, 0);
    assert!(!frame.has_pending_query());
    #[cfg(feature = "profiling")]
    {
        let events = profile.snapshot().owned_execution_events;
        assert_eq!(events.get("public_field_query_fallback"), Some(&1));
        assert!(!events.contains_key("public_field_completed_without_query"));
    }
    drop(execution);
    assert_eq!(
        context
            .eval("fieldCalls===0&&!('x' in fieldTarget)")
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn public_fields_keep_own_definition_and_observable_special_fallback_order() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    assert_eq!(context.eval(r#"(function(){
        let trace='',sets=0,marker={answer:42};
        Object.defineProperty(Object.prototype,'field',{configurable:true,set(){sets++;throw 99}});
        try {
            const key={toString(){trace+='key;';return 'field'}};
            const ordinary={field:marker,[key]:marker};
            let target={},proxy=new Proxy(target,{defineProperty(t,k,d){
                trace+='proxy;';const nested={field:42};
                if(nested.field!==42||d.value!==marker||!d.writable||!d.enumerable||!d.configurable)throw 98;
                return Reflect.defineProperty(t,k,d);
            }});
            class Base{constructor(){return proxy}} class Derived extends Base{field=marker}
            new Derived;
            const array=[1,2,3];class ArrayBase{constructor(){return array}}
            class ArrayDerived extends ArrayBase{length={valueOf(){trace+='length;';return 1}}}
            try{new ArrayDerived;return false}catch(e){if(!(e instanceof TypeError))return false}
            const typed=new Uint8Array(1);class TypedBase{constructor(){return typed}}
            class TypedDerived extends TypedBase{0={valueOf(){trace+='typed;';return 42}}}
            new TypedDerived;
            const abrupt=new Proxy({}, {defineProperty(){trace+='throw;';throw marker}});
            class AbruptBase{constructor(){return abrupt}}
            class AbruptDerived extends AbruptBase{field=17}
            try{new AbruptDerived;return false}catch(e){if(e!==marker)return false}finally{trace+='finally;'}
            return ordinary.field===marker&&target.field===marker&&array.length===3&&typed[0]===42&&sets===0&&
                trace==='key;proxy;length;length;typed;throw;finally;';
        } finally {delete Object.prototype.field;}
    })()"#).unwrap(), Value::Bool(true));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn public_field_rejection_uses_frame_realm_and_keeps_fault_pc() {
    let runtime = Runtime::new();
    let mut defining = runtime.new_context();
    let mut caller = runtime.new_context();
    let prototype = defining.eval("TypeError.prototype").unwrap();
    let Value::Object(target) = defining.eval("Object.preventExtensions({})").unwrap() else {
        panic!("ordinary object")
    };
    let marker = runtime.new_object(None).unwrap();
    let (mut execution, id) = fixture(&runtime, &mut defining, &caller, 1);
    let result = start_public_field(
        &runtime,
        &mut execution,
        id,
        target.clone(),
        runtime.intern_property_key("x").unwrap(),
        JsValue::Object(marker.clone().into_handle()),
        0,
    )
    .unwrap();
    let CallStep::Complete(Completion::Throw(thrown)) = result else {
        panic!("expected definition rejection")
    };
    let Value::Object(error) = runtime.root_value(&thrown).unwrap() else {
        panic!("error object")
    };
    runtime.release_jsvalue(thrown).unwrap();
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap().map(Value::Object),
        Some(prototype)
    );
    assert!(
        runtime
            .get_own_property(&target, &runtime.intern_property_key("x").unwrap())
            .unwrap()
            .is_none()
    );
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(marker.object_id()),
        Ok(1)
    );
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(frame.property_generation, 1);
    assert_eq!(frame.resume_pc, frame.fault_pc);
    assert!(frame.active_frame.is_materialized());
    drop(execution);
    assert_eq!(
        caller
            .get_property(&error, &runtime.intern_property_key("message").unwrap())
            .unwrap(),
        Value::String(crate::engine::value::JsString::from_static(
            "property is not configurable"
        ))
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn failed_public_field_domain_check_leaves_target_unmodified_and_cleans_request() {
    let runtime = Runtime::new();
    runtime.set_gc_policy(GcPolicy::Manual);
    let mut context = runtime.new_context();
    let caller = runtime.new_context();
    let (mut execution, id) = fixture(&runtime, &mut context, &caller, 1);
    let target = runtime.new_object(None).unwrap();
    let marker = runtime.new_object(None).unwrap();
    let foreign = Runtime::new();
    let foreign_key = foreign.intern_property_key("x").unwrap();
    let result = start_public_field(
        &runtime,
        &mut execution,
        id,
        target.clone(),
        foreign_key,
        JsValue::Object(marker.clone().into_handle()),
        0,
    );
    let Err(error) = result else {
        panic!("expected domain check failure")
    };
    assert!(
        error
            .message()
            .contains("property key belongs to another runtime")
    );
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(marker.object_id()),
        Ok(1)
    );
    assert!(
        runtime
            .get_own_property(&target, &runtime.intern_property_key("x").unwrap())
            .unwrap()
            .is_none()
    );
    let frame = execution.frames.current_mut(id).unwrap();
    assert_eq!(frame.property_generation, 1);
    assert_eq!(frame.resume_pc, 0);
    assert!(!frame.has_pending_query());
    assert!(!execution.query_storage.has_cached_entry());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(target.object_id()),
        Ok(1)
    );
    drop(execution);
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn public_field_preserves_cleanup_and_ready_gc_service_with_live_definition() {
    let runtime = Runtime::new();
    runtime.set_gc_policy(GcPolicy::Manual);
    let mut context = runtime.new_context();
    let caller = runtime.new_context();
    let Value::Object(garbage) = context
        .eval("(function(){const o={};o.self=o;return o})()")
        .unwrap()
    else {
        panic!("unreachable cycle")
    };
    let garbage_id = garbage.object_id();
    drop(garbage);
    let (mut execution, id) = fixture(&runtime, &mut context, &caller, 1);
    let target = runtime.new_object(None).unwrap();
    let marker = runtime.new_object(None).unwrap();
    let object = target.clone();
    let key = runtime.intern_property_key("x").unwrap();
    let value = JsValue::Object(marker.clone().into_handle());
    let pending = runtime.new_object(None).unwrap().into_handle();
    let deferred = runtime.new_object(None).unwrap();
    let deferred_id = deferred.object_id();
    {
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .queue_release_for_test(RawId::Object(pending))
            .unwrap();
        drop(deferred);
    }
    runtime.set_gc_policy(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    assert!(runtime.0.deferred_references.has_pending());
    assert!(runtime.0.state.borrow().heap.has_pending_zero_cleanup());
    assert!(matches!(
        start_public_field(&runtime, &mut execution, id, object, key, value, 0).unwrap(),
        CallStep::Entered
    ));
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.0.state.borrow().heap.has_pending_zero_cleanup());
    for id in [pending, deferred_id] {
        assert!(runtime.0.state.borrow().heap.object(id).is_err());
    }
    // The synchronous kernel drains the same releases, while automatic cycle
    // service remains the next ready driver boundary with its result rooted.
    assert!(runtime.0.state.borrow().heap.object(garbage_id).is_ok());
    runtime.collect_if_requested().unwrap();
    assert!(runtime.0.state.borrow().heap.object(garbage_id).is_err());
    assert!(runtime.0.gc_pressure.remaining.get() > 0);
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(marker.object_id()),
        Ok(2)
    );
    assert!(
        execution
            .frames
            .current_mut(id)
            .unwrap()
            .active_frame
            .is_materialized()
    );
    drop(execution);
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}
