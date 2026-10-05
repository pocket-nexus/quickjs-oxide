use super::*;
use crate::engine::api::Value;

#[test]
fn get_invariant_consumes_empty_own_proxy_prefix_without_descriptor_request() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context
        .eval("new Proxy(new Proxy(Object.freeze({x:7}),{}),{get(){return 7}})")
        .unwrap()
    else {
        panic!()
    };
    let key = runtime.intern_property_key("x").unwrap();
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let ProxyGetStep::Effect(StateReadEffect::Get(ProxyGetEffect::Call(mut resume))) =
        ProxyGetStep::start_in_state(
            &runtime,
            &mut runtime.0.state.borrow_mut(),
            context.realm,
            proxy.object_id(),
            key.atom(),
            &JsValue::Undefined,
            Vec::new(),
        )
        .unwrap()
    else {
        panic!("the selected get trap is the only callback")
    };
    let (target, receiver, arguments) = resume.take_call(&runtime);
    drop(target);
    runtime.release_jsvalue(receiver).unwrap();
    for value in arguments {
        runtime.release_jsvalue(value).unwrap();
    }
    let step = resume
        .resume(&runtime, Completion::Return(JsValue::Int(7)))
        .unwrap();
    assert!(matches!(
        step,
        ProxyGetStep::Complete(Completion::Return(JsValue::Int(7)))
    ));
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    #[cfg(feature = "profiling")]
    assert!(
        !profile
            .snapshot()
            .owned_execution_events
            .contains_key("legacy_method_resume_allocation")
    );
}

#[test]
fn abandoned_get_invariant_releases_exact_selected_own_getter() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context.eval(
        "new Proxy(new Proxy({x:7},{get getOwnPropertyDescriptor(){throw 42}}),{get(){return 7}})",
    ).unwrap() else { panic!() };
    let key = runtime.intern_property_key("x").unwrap();
    let ProxyGetStep::Effect(StateReadEffect::Get(ProxyGetEffect::Call(mut resume))) =
        ProxyGetStep::start(&runtime, context.realm, proxy, key, JsValue::Undefined).unwrap()
    else {
        panic!()
    };
    let (target, receiver, arguments) = resume.take_call(&runtime);
    drop(target);
    runtime.release_jsvalue(receiver).unwrap();
    for value in arguments {
        runtime.release_jsvalue(value).unwrap();
    }
    let step = resume
        .resume(&runtime, Completion::Return(JsValue::Int(7)))
        .unwrap();
    assert!(matches!(
        step,
        ProxyGetStep::Effect(StateReadEffect::Get(ProxyGetEffect::PreparedDescriptor(_)))
    ));
    assert!(runtime.0.proxy_method_depth.get() > 0);
    let owners = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    step.release_in_state(&runtime, &mut state).unwrap();
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    assert!(!runtime.is_poisoned());
}

#[test]
fn synchronous_forwarding_owns_no_runtime_or_resume() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context.eval("new Proxy(new Proxy({x:42},{}),{})").unwrap() else {
        panic!()
    };
    let key = runtime.intern_property_key("x").unwrap();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    #[cfg(feature = "profiling")]
    let profile = crate::engine::api::profiling::CostProfile::start();
    let mut state = runtime.0.state.borrow_mut();
    let step = ProxyGetStep::start_in_state(
        &runtime,
        &mut state,
        context.realm,
        proxy.object_id(),
        key.atom(),
        &JsValue::Undefined,
        Vec::new(),
    )
    .unwrap();
    assert!(matches!(
        step,
        ProxyGetStep::Complete(Completion::Return(JsValue::Int(42)))
    ));
    assert_eq!(owners, std::rc::Rc::strong_count(&runtime.0));
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
    #[cfg(feature = "profiling")]
    assert!(
        !profile
            .snapshot()
            .owned_execution_events
            .contains_key("get_resume_allocation")
    );
}

#[test]
fn abandoned_proxy_method_releases_roots_and_depth_guard() {
    let runtime = Runtime::new();
    let weak = std::rc::Rc::downgrade(&runtime.0);
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context
        .eval("new Proxy({}, {get get(){return undefined}})")
        .unwrap()
    else {
        panic!()
    };
    let id = proxy.object_id();
    let step = ProxyGetStep::start(
        &runtime,
        context.realm,
        proxy,
        runtime.intern_property_key("x").unwrap(),
        JsValue::Undefined,
    )
    .unwrap();
    assert_eq!(runtime.0.proxy_method_depth.get(), 1);
    step.release_in_state(&runtime, &mut runtime.0.state.borrow_mut())
        .unwrap();
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    runtime.run_gc().unwrap();
    assert!(runtime.0.state.borrow().heap.object(id).is_err());
    assert!(!runtime.0.deferred_references.has_pending());
    drop(context);
    drop(runtime);
    assert!(weak.upgrade().is_none());
}

#[test]
fn mismatched_proxy_reply_releases_selected_request_and_method_depth() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context
        .eval("new Proxy({}, {get get(){return undefined}})")
        .unwrap()
    else {
        panic!()
    };
    let ProxyGetStep::Effect(StateReadEffect::Get(ProxyGetEffect::Read(resume))) =
        ProxyGetStep::start(
            &runtime,
            context.realm,
            proxy,
            runtime.intern_property_key("x").unwrap(),
            JsValue::Undefined,
        )
        .unwrap()
    else {
        panic!("real method getter")
    };
    assert_eq!(runtime.0.proxy_method_depth.get(), 1);
    assert!(
        resume
            .descriptor(&runtime, NativeConversion::Value(None))
            .is_err()
    );
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn receiver_retain_failure_releases_selected_method_request() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(proxy) = context
        .eval("new Proxy({}, {get get(){return undefined}})")
        .unwrap()
    else {
        panic!()
    };
    let key = runtime.intern_property_key("x").unwrap();
    let receiver = runtime.new_object(None).unwrap();
    let id = receiver.object_id();
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(crate::engine::heap::RawId::Object(id), u32::MAX);
    let result = ProxyGetStep::start_in_state(
        &runtime,
        &mut state,
        context.realm,
        proxy.object_id(),
        key.atom(),
        &JsValue::Object(id),
        Vec::new(),
    );
    state
        .heap
        .set_strong_count_for_test(crate::engine::heap::RawId::Object(id), 1);
    assert!(result.is_err());
    assert_eq!(runtime.0.proxy_method_depth.get(), 0);
    assert!(!runtime.0.poisoned.get());
    assert!(!runtime.0.deferred_references.has_pending());
}
