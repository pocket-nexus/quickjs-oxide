//! Actual native/query consumers preserve effects and abrupt-completion timing.
use super::*;
fn text(context: &mut crate::engine::api::Context, source: &str) -> JsString {
    let Value::String(value) = context.eval(source).unwrap() else {
        panic!("text result")
    };
    value
}
#[test]
fn date_constructor_real_callbacks_keep_all_conversion_before_prototype_order() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        text(
            &mut context,
            "var dcLog=[]; var dcTarget=new Proxy(function(){},{get(t,k){if(k==='prototype'){dcLog.push('prototype');return Date.prototype;}return t[k];}}); var dcField=i=>({valueOf(){dcLog.push(i);return i===0?NaN:i;}}); Reflect.construct(Date,[dcField(0),dcField(1),dcField(2),dcField(3),dcField(4),dcField(5),dcField(6),dcField(7)],dcTarget); JSON.stringify(dcLog)"
        ),
        JsString::from_static("[0,1,2,3,4,5,6,\"prototype\"]")
    );
    assert!(!runtime.is_poisoned());
}
#[test]
fn date_parse_and_utc_real_bound_getters_keep_throws_and_native_observations() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        text(
            &mut context,
            "var dsLog=[]; var dsError={}; var dsParse={get [Symbol.toPrimitive](){dsLog.push('get');return (function(h){dsLog.push(h);return '2000-01-01T00:00:00Z';}).bind(null);}}; var dsYear={valueOf(){dsLog.push('year');throw dsError;}}; Date.parse(dsParse); try {Date.UTC(dsYear,{valueOf(){dsLog.push('month');return 1;}});}catch(e){dsLog.push(e===dsError);} JSON.stringify(dsLog)"
        ),
        JsString::from_static("[\"get\",\"string\",\"year\",true]")
    );
}
#[test]
fn date_constructor_prototype_get_revoke_and_child_throw_are_not_replayed() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    assert_eq!(
        text(
            &mut context,
            "var drLog=[]; var dr = Proxy.revocable(function(){},{get(t,k){if(k==='prototype'){drLog.push('get');dr.revoke();return null;}return t[k];}}); try {Reflect.construct(Date,[0],dr.proxy);}catch(e){drLog.push(e instanceof TypeError);} var drThrown={}; var drTarget=new Proxy(function(){},{get(t,k){if(k==='prototype'){drLog.push('throw');throw drThrown;}return t[k];}}); try{Reflect.construct(Date,[1],drTarget);}catch(e){drLog.push(e===drThrown);} JSON.stringify(drLog)"
        ),
        JsString::from_static("[\"get\",true,\"throw\",true]")
    );
}
#[test]
fn date_constructor_primitive_newtarget_uses_same_value_read_and_fallback_realm() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Int(7),
        vec![],
    )
    .unwrap();
    let DateConstructorStep::Read {
        receiver,
        key,
        resume,
        ..
    } = step
    else {
        panic!("primitive newTarget read")
    };
    let read = state
        .prepare_value_read_in_state(
            &runtime.0.poisoned,
            runtime.domain_id(),
            context.realm,
            &receiver,
            key,
            None,
        )
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, receiver)
        .unwrap();
    let crate::engine::object::ReadStep::Ready(crate::engine::object::OwnedRead::Complete(value)) =
        read
    else {
        panic!("ordinary primitive prototype property")
    };
    let step = resume
        .resume_in_state(
            &mut state,
            &runtime.0.poisoned,
            Completion::Return(value.unwrap_or(JsValue::Undefined)),
        )
        .unwrap();
    assert!(matches!(
        step,
        DateConstructorStep::CyclePublished(Completion::Return(JsValue::Object(_)))
    ));
    retired(step, &mut state, &runtime);
}
#[test]
fn date_parse_public_adapter_uses_shared_raw_core_and_retires_snapshot_once() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let input = object(
        &mut context,
        "({toString(){return '2000-01-01T00:00:00Z';}})",
    );
    let before = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(input.object_id())
        .unwrap();
    let completion = runtime
        .call_date_constructor_native(
            context.realm,
            DateNativeKind::Parse,
            &NativeInvocation::Call {
                this_value: JsValue::Undefined,
            },
            &NativeArguments {
                readable: vec![JsValue::Object(input.object_id())],
                actual_arg_count: 1,
            },
        )
        .unwrap();
    let Completion::Return(value) = completion else {
        panic!("public Date.parse return")
    };
    assert_eq!(value, JsValue::Float(946684800000.0));
    runtime.release_jsvalue(value).unwrap();
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(input.object_id())
            .unwrap(),
        before
    );
    assert!(!runtime.is_poisoned());
}

#[test]
fn date_constructor_native_family_uses_state_body_for_call_parse_utc_and_construct() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    assert_eq!(context.eval("typeof Date({valueOf(){throw 1;}})==='string' && Date.parse('2000-01-01T00:00:00Z')===946684800000 && Date.UTC(2000,0,1)===946684800000 && new Date(7).getTime()===7").unwrap(), Value::Bool(true));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert!(!runtime.is_poisoned());
    #[cfg(feature = "profiling")]
    assert_eq!(
        profile
            .snapshot()
            .owned_execution_events
            .get("date_constructor_state_start"),
        Some(&4)
    );
}
#[test]
fn date_constructor_to_primitive_bigint_error_is_actual_fresh_publication() {
    let (runtime, _, _) = runtime(false);
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap();
    let input = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let step = start(
        &mut state,
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Object(target.object_id()),
        vec![JsValue::Object(input.object_id())],
    )
    .unwrap();
    let DateConstructorStep::Primitive { value, resume } = step else {
        panic!("single Primitive")
    };
    state
        .release_owned_jsvalue(&runtime.0.poisoned, value)
        .unwrap();
    let step = resume
        .primitive_in_state(
            &mut state,
            &runtime.0.poisoned,
            runtime.0.host_services.as_ref(),
            Completion::Return(JsValue::ShortBigInt(7)),
        )
        .unwrap();
    assert!(matches!(
        step,
        DateConstructorStep::CyclePublished(Completion::Throw(JsValue::Object(_)))
    ));
    assert_eq!(state.heap.object_strong_count(target.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(input.object_id()), Ok(1));
    retired(step, &mut state, &runtime);
}

#[test]
fn date_constructor_root_registration_rejection_keeps_effects_unrun_and_retires_raw_snapshot() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let input = object(
        &mut context,
        "var dateRegistrationEffects=0; ({toString(){dateRegistrationEffects++;return '2000-01-01';}})",
    );
    let count = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(input.object_id())
        .unwrap();
    runtime.0.raw_execution_owners.set(usize::MAX);
    let result = runtime.call_date_constructor_native(
        context.realm,
        DateNativeKind::Parse,
        &NativeInvocation::Call {
            this_value: JsValue::Undefined,
        },
        &NativeArguments {
            readable: vec![JsValue::Object(input.object_id())],
            actual_arg_count: 1,
        },
    );
    runtime.0.raw_execution_owners.set(0);
    assert!(result.is_err());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(input.object_id())
            .unwrap(),
        count
    );
    assert_eq!(
        context.eval("dateRegistrationEffects").unwrap(),
        Value::Int(0)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}
#[test]
fn date_constructor_fallback_prototype_uses_foreign_defining_realm_after_get() {
    let (runtime, clock, _) = runtime(false);
    let caller = runtime.new_context().unwrap();
    let mut foreign = runtime.new_context().unwrap();
    let target = object(
        &mut foreign,
        "var foreignDateTarget = function ForeignDateTarget() {}; foreignDateTarget.prototype=null; foreignDateTarget",
    );
    let mut state = runtime.0.state.borrow_mut();
    let expected = state.heap.context(foreign.realm).unwrap().date_prototype;
    let step = start(
        &mut state,
        &runtime,
        caller.realm,
        DateNativeKind::Constructor,
        JsValue::Object(target.object_id()),
        vec![],
    )
    .unwrap();
    let DateConstructorStep::Read {
        receiver,
        key,
        resume,
        realm,
    } = step
    else {
        panic!("prototype request")
    };
    assert_eq!(realm, caller.realm);
    let read = state
        .prepare_value_read_in_state(
            &runtime.0.poisoned,
            runtime.domain_id(),
            realm,
            &receiver,
            key,
            None,
        )
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, receiver)
        .unwrap();
    let crate::engine::object::ReadStep::Ready(crate::engine::object::OwnedRead::Complete(Some(
        JsValue::Null,
    ))) = read
    else {
        panic!("prototype data lookup")
    };
    let step = resume
        .resume_in_state(
            &mut state,
            &runtime.0.poisoned,
            Completion::Return(JsValue::Null),
        )
        .unwrap();
    let DateConstructorStep::CyclePublished(Completion::Return(JsValue::Object(date))) = step
    else {
        panic!("fresh foreign prototype Date")
    };
    assert_eq!(
        state
            .heap
            .shape(state.heap.object(date).unwrap().shape)
            .unwrap()
            .prototype(),
        expected
    );
    assert_eq!(state.heap.date_value(date), Ok(42.0));
    assert_eq!(clock.get(), 1);
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(date))
        .unwrap();
}

#[test]
fn date_constructor_public_finish_preserves_typed_getter_retain_failure() {
    let (runtime, _, _) = runtime(false);
    let mut context = runtime.new_context().unwrap();
    let target = object(
        &mut context,
        "var dateTypedGetterRan=false; var dateTypedGetter=function(){dateTypedGetterRan=true; return Date.prototype}; var dateTypedTarget={}; Object.defineProperty(dateTypedTarget,'prototype',{get:dateTypedGetter}); dateTypedTarget",
    );
    let getter = object(&mut context, "dateTypedGetter");
    let (original_getter, original_target) = {
        let state = runtime.0.state.borrow();
        (
            state.heap.object_strong_count(getter.object_id()).unwrap(),
            state.heap.object_strong_count(target.object_id()).unwrap(),
        )
    };
    let step = start(
        &mut runtime.0.state.borrow_mut(),
        &runtime,
        context.realm,
        DateNativeKind::Constructor,
        JsValue::Object(target.object_id()),
        vec![],
    )
    .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(getter.object_id()), u32::MAX);
    let result = super::super::finish(&runtime, context.realm, step);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(getter.object_id()), original_getter);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(!runtime.is_poisoned());
    {
        let state = runtime.0.state.borrow();
        assert_eq!(
            state.heap.object_strong_count(target.object_id()).unwrap(),
            original_target
        );
        assert!(state.active_frames.is_empty());
    }
    assert_eq!(
        context.eval("dateTypedGetterRan").unwrap(),
        Value::Bool(false)
    );
}
