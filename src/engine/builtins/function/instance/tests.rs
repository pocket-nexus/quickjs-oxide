use super::*;
use crate::engine::value::Value;

#[test]
fn ordinary_instanceof_uses_current_method_and_prototype_without_callbacks() {
    let source = r#"(() => {
        let calls=0;
        function C() {}
        const original=C.prototype, object=new C();
        for(let i=0;i<20;i++) if(!(object instanceof C)) return false;
        C.prototype={};
        if(object instanceof C) return false;
        C.prototype=original;
        if(!(object instanceof C)) return false;
        Object.defineProperty(C,Symbol.hasInstance,{configurable:true,value(v){calls++;return v===object}});
        if(!(object instanceof C) || calls!==1) return false;
        delete C[Symbol.hasInstance];
        if(!(object instanceof C) || calls!==1) return false;
        Object.defineProperty(C,Symbol.hasInstance,{configurable:true,value:null});
        if(!(object instanceof C) || 1 instanceof C) return false;
        delete C[Symbol.hasInstance];
        Object.setPrototypeOf(C, { [Symbol.hasInstance](v){calls++;return false} });
        return !(object instanceof C) && calls===2;
    })()"#;
    assert_eq!(
        Runtime::new().new_context().eval(source).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn ordinary_instanceof_misses_preserve_selection_order_and_error_identity() {
    let cases = [
        r#"(()=>{let log='';const marker={};function C(){}Object.defineProperty(C,Symbol.hasInstance,{get(){log+='h';throw marker}});const p=new Proxy({}, {getPrototypeOf(){log+='p';return null}});try{p instanceof C}catch(e){return e===marker&&log==='h'}return false})()"#,
        r#"(()=>{let log='';const marker={};const C=new Proxy(function(){},{get(t,k,r){if(k===Symbol.hasInstance){log+='h';return undefined}if(k==='prototype'){log+='p';throw marker}return Reflect.get(t,k,r)}});const p=new Proxy({}, {getPrototypeOf(){log+='o';return null}});if(0 instanceof C)return false;try{p instanceof C}catch(e){return e===marker&&log==='hhp'}return false})()"#,
        r#"(()=>{function C(){}const o=new C(),bound=C.bind(null);if(!(o instanceof bound))return false;C.prototype=1;if(1 instanceof C)return false;try{o instanceof C}catch(e){return e instanceof TypeError}return false})()"#,
        r#"(()=>{function C(){}const o=new C();Object.defineProperty(C,Symbol.hasInstance,{value:1});try{o instanceof C}catch(e){return e instanceof TypeError}return false})()"#,
        r#"(()=>{function C(){}const o=new C();const get={get [Symbol.hasInstance](){return Function.prototype[Symbol.hasInstance]}};Object.setPrototypeOf(C,get);return o instanceof C})()"#,
        r#"(()=>{const f=()=>0;if(0 instanceof f)return false;try{({}) instanceof f}catch(e){return e instanceof TypeError}return false})()"#,
        r#"(()=>{const rhs={};try{1 instanceof rhs}catch(e){return e instanceof TypeError}return false})()"#,
    ];
    for source in cases {
        assert_eq!(
            Runtime::new().new_context().eval(source).unwrap(),
            Value::Bool(true),
            "{source}"
        );
    }
}

#[test]
fn ordinary_instanceof_declines_cleanup_saturation_and_stale_inputs() {
    use crate::engine::heap::RawId;
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let Value::Object(target) = context.eval("globalThis.C=function C(){};C").unwrap() else {
        panic!("constructor");
    };
    let Value::Object(candidate) = context.eval("new C()").unwrap() else {
        panic!("candidate");
    };
    drop(context.eval("C[Symbol.hasInstance]").unwrap());
    let _candidate_owner = candidate.clone();
    let value = JsValue::Object(candidate.object_id());
    let ids = {
        let state = runtime.0.state.borrow();
        let method = state.well_known_symbols[&WellKnownSymbol::HasInstance];
        let Some(RawValue::Object(method)) =
            borrowed_ordinary_data(&state.heap, target.object_id(), method).unwrap()
        else {
            panic!("intrinsic method");
        };
        let key = state
            .pinned_atoms
            .get(crate::engine::atom::pinned::PinnedAtom::Prototype);
        let Some(RawValue::Object(prototype)) =
            borrowed_ordinary_data(&state.heap, target.object_id(), key).unwrap()
        else {
            panic!("prototype");
        };
        [
            target.object_id(),
            candidate.object_id(),
            *method,
            *prototype,
        ]
    };
    assert_eq!(
        try_ordinary_instanceof(&runtime, &value, target.object_id(), true),
        Some(true)
    );
    for id in ids {
        let original = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(id)
            .unwrap();
        for count in [
            u32::MAX - INSTANCE_PROTOCOL_ROOT_HEADROOM,
            u32::MAX - 1,
            u32::MAX,
        ] {
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(id), count);
            assert_eq!(
                try_ordinary_instanceof(&runtime, &value, target.object_id(), true),
                None
            );
            assert_eq!(
                runtime.0.state.borrow().heap.object_strong_count(id),
                Ok(count)
            );
        }
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), original);
    }
    let doomed = runtime.new_object(None).unwrap();
    {
        let _state = runtime.0.state.borrow();
        drop(doomed);
    }
    assert_eq!(
        try_ordinary_instanceof(&runtime, &value, target.object_id(), true),
        None
    );
    runtime.drain_deferred_references().unwrap();
    assert_eq!(
        try_ordinary_instanceof(&runtime, &value, target.object_id(), true),
        Some(true)
    );
    let stale = runtime.new_object(None).unwrap();
    let stale_id = stale.object_id();
    drop(stale);
    let replacement = runtime.new_object(None).unwrap();
    assert_ne!(stale_id, replacement.object_id());
    assert_eq!(
        try_ordinary_instanceof(&runtime, &value, stale_id, true),
        None
    );
    assert_eq!(
        try_ordinary_instanceof(
            &runtime,
            &JsValue::Object(stale_id),
            target.object_id(),
            true
        ),
        None
    );
}

#[test]
fn ordinary_instanceof_shared_start_handles_aliased_owners_and_final_owners() {
    let cases = [
        // The candidate is the callable itself, with an own intrinsic method
        // so changing its [[Prototype]] does not change method selection.
        (
            r#"globalThis.C=function C(){};Object.defineProperty(C,Symbol.hasInstance,{value:Function.prototype[Symbol.hasInstance]});Object.setPrototypeOf(C,C.prototype);globalThis.pair=C;pair instanceof C"#,
            true,
        ),
        // OrdinaryHasInstance excludes its candidate from the chain.
        (
            r#"globalThis.C=function C(){};globalThis.pair=C.prototype;pair instanceof C"#,
            false,
        ),
        // Both the callable target and selected intrinsic are one object.
        (
            r#"globalThis.C=Function.prototype[Symbol.hasInstance];C.prototype={};globalThis.pair=Object.create(C.prototype);pair instanceof C"#,
            true,
        ),
    ];
    for (setup, expected) in cases {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert_eq!(context.eval(setup).unwrap(), Value::Bool(expected));
        let Value::Object(target) = context.eval("C").unwrap() else {
            panic!("target");
        };
        let Value::Object(candidate) = context.eval("pair").unwrap() else {
            panic!("candidate");
        };
        let target_count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(target.object_id())
            .unwrap();
        let candidate_count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(candidate.object_id())
            .unwrap();
        let step = InstanceStep::start(
            &runtime,
            context.realm,
            JsValue::Object(candidate.clone().into_handle()),
            target.clone(),
            true,
        )
        .unwrap();
        assert!(
            matches!(step, InstanceStep::Complete(Completion::Return(JsValue::Bool(found))) if found == expected)
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(target.object_id())
                .unwrap(),
            target_count
        );
        assert_eq!(
            runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(candidate.object_id())
                .unwrap(),
            candidate_count
        );
        #[cfg(feature = "profiling")]
        {
            let profile = crate::engine::api::profiling::CostProfile::start();
            assert_eq!(
                context.eval("pair instanceof C").unwrap(),
                Value::Bool(expected)
            );
            assert_eq!(
                profile
                    .snapshot()
                    .owned_execution_events
                    .get("instanceof.completed_without_continuation"),
                Some(&1)
            );
        }
        #[cfg(not(feature = "profiling"))]
        assert_eq!(
            context.eval("pair instanceof C").unwrap(),
            Value::Bool(expected)
        );
    }

    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let Value::Object(target) = context
        .eval("globalThis.C=function C(){};C[Symbol.hasInstance];C")
        .unwrap()
    else {
        panic!("target");
    };
    let Value::Object(prototype) = context.eval("C.prototype").unwrap() else {
        panic!("prototype");
    };
    let candidate = runtime.new_object(Some(&prototype)).unwrap();
    let candidate_id = candidate.object_id();
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(candidate_id),
        Ok(1)
    );
    let step = InstanceStep::start(
        &runtime,
        context.realm,
        JsValue::Object(candidate.into_handle()),
        target.clone(),
        true,
    )
    .unwrap();
    assert!(matches!(step, InstanceStep::Read { .. }));
    assert!(matches!(
        finish(&runtime, context.realm, step).unwrap(),
        Completion::Return(JsValue::Bool(true))
    ));
    assert!(runtime.0.state.borrow().heap.object(candidate_id).is_err());
    assert_eq!(
        context.eval("new C() instanceof C").unwrap(),
        Value::Bool(true)
    );

    let Value::Object(target) = context.eval("(function(){})").unwrap() else {
        panic!("final target");
    };
    let target_id = target.object_id();
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(target_id),
        Ok(1)
    );
    let step = InstanceStep::start(&runtime, context.realm, JsValue::Int(0), target, true).unwrap();
    assert!(matches!(step, InstanceStep::Read { .. }));
    assert!(matches!(
        finish(&runtime, context.realm, step).unwrap(),
        Completion::Return(JsValue::Bool(false))
    ));
    assert!(runtime.0.state.borrow().heap.object(target_id).is_err());
}

#[test]
fn ordinary_instanceof_preserves_intrinsic_budget_and_cross_realm_selection() {
    let runtime = Runtime::new();
    let mut foreign = runtime.new_context();
    let Value::Object(target) = foreign
        .eval("globalThis.C=function C(){};C[Symbol.hasInstance];C")
        .unwrap()
    else {
        panic!("foreign target");
    };
    let Value::Object(prototype) = foreign.eval("C.prototype").unwrap() else {
        panic!("foreign prototype");
    };
    let candidate = runtime.new_object(Some(&prototype)).unwrap();
    let _extra_owner = candidate.clone();
    let value = JsValue::Object(candidate.object_id());
    assert_eq!(
        try_ordinary_instanceof(&runtime, &value, target.object_id(), true),
        Some(true)
    );
    assert_eq!(
        try_ordinary_instanceof(&runtime, &value, target.object_id(), false),
        None
    );
    let mut local = runtime.new_context();
    assert!(matches!(
        InstanceStep::start(
            &runtime,
            local.realm,
            JsValue::Object(candidate.clone().into_handle()),
            target.clone(),
            true
        )
        .unwrap(),
        InstanceStep::Complete(Completion::Return(JsValue::Bool(true)))
    ));
    // Nullish @@hasInstance performs OrdinaryHasInstance without entering an
    // intrinsic; its successful result does not require a native-call budget.
    drop(
        foreign
            .eval("Object.defineProperty(C,Symbol.hasInstance,{value:null})")
            .unwrap(),
    );
    assert_eq!(
        try_ordinary_instanceof(&runtime, &value, target.object_id(), false),
        Some(true)
    );

    drop(
        local
            .eval("globalThis.C=function C(){};globalThis.pair=new C();pair instanceof C")
            .unwrap(),
    );
    runtime.set_recursion_limit(1);
    // The current script occupies the only execution frame. The intrinsic
    // still throws through its original driver and can be caught in JS.
    assert_eq!(
        local
            .eval("try { pair instanceof C; false } catch(e) { e.message==='stack overflow' }")
            .unwrap(),
        Value::Bool(true)
    );
}

#[cfg(feature = "profiling")]
#[test]
fn ordinary_instanceof_real_opcode_finishes_without_native_activation() {
    use crate::engine::api::profiling::CostProfile;
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    drop(
        context
            .eval("globalThis.C=function C(){};globalThis.pair=new C();pair instanceof C")
            .unwrap(),
    );
    let profile = CostProfile::start();
    assert_eq!(context.eval("(function(){let result=0;for(let i=0;i<20;i++){if(pair instanceof C)result++}return result})()").unwrap(), Value::Int(20));
    let snapshot = profile.snapshot();
    assert_eq!(
        snapshot
            .owned_execution_events
            .get("instanceof.completed_without_continuation"),
        Some(&20)
    );
    assert_eq!(
        snapshot
            .owned_execution_events
            .get("execute.action.predicate"),
        Some(&20)
    );
    assert!(
        !snapshot
            .owned_execution_events
            .contains_key("native_activation_prepared")
    );
    assert!(
        !snapshot
            .owned_execution_events
            .contains_key("native_call_direct_wait")
    );
}

#[test]
fn instanceof_chain_keeps_full_generational_identity() {
    let runtime = Runtime::new();
    let stale = runtime.new_object(None).unwrap();
    let stale_id = stale.object_id();
    drop(stale);
    let replacement = runtime.new_object(None).unwrap();
    assert_eq!(
        stale_id.debug_index(),
        replacement.object_id().debug_index()
    );
    assert_ne!(
        stale_id.debug_generation(),
        replacement.object_id().debug_generation()
    );
    let candidate = runtime.new_object(Some(&replacement)).unwrap();
    let state = runtime.0.state.borrow();
    assert_eq!(
        walk_ordinary_chain(&state.heap, candidate.object_id(), stale_id).unwrap(),
        ChainWalk::Complete(false)
    );
    assert_eq!(
        walk_ordinary_chain(&state.heap, candidate.object_id(), replacement.object_id()).unwrap(),
        ChainWalk::Complete(true)
    );
    assert!(matches!(
        walk_ordinary_chain(&state.heap, stale_id, replacement.object_id()),
        Err(HeapError::Stale { .. })
    ));
}

#[test]
fn instanceof_chain_bounds_a_borrow_and_resumes_from_progress() {
    let runtime = Runtime::new();
    let expected = runtime.new_object(None).unwrap();
    let mut objects = vec![expected];
    for _ in 0..40 {
        objects.push(runtime.new_object(objects.last()).unwrap());
    }
    let state = runtime.0.state.borrow();
    let expected = objects[0].object_id();
    assert_eq!(
        walk_ordinary_chain(&state.heap, objects[40].object_id(), expected).unwrap(),
        ChainWalk::Protocol(objects[8].object_id())
    );
    assert_eq!(
        walk_ordinary_chain(&state.heap, objects[8].object_id(), expected).unwrap(),
        ChainWalk::Complete(true)
    );
    // OrdinaryHasInstance excludes the candidate itself.
    assert_eq!(
        walk_ordinary_chain(&state.heap, expected, expected).unwrap(),
        ChainWalk::Complete(false)
    );
}

#[test]
fn instanceof_chain_preserves_callbacks_and_exceptions() {
    let cases = [
        // A long chain crosses the batch boundary; distinct prototypes miss.
        r#"(()=>{function C(){} function D(){} let o=C.prototype;for(let i=0;i<80;i++)o=Object.create(o);return o instanceof C && !(o instanceof D) && !(C.prototype instanceof C) && !(Object.create(null) instanceof C)})()"#,
        // Proxy callback mutates the original candidate chain. Resume from its
        // returned value rather than restarting from the changed candidate.
        r#"(()=>{function C(){} let n=0,o;const p=new Proxy({}, {getPrototypeOf(){n++;Object.setPrototypeOf(o,null);return Object.create(C.prototype)}});o=Object.create(Object.create(p));return o instanceof C && n===1 && !(o instanceof C)})()"#,
        // Matching the expected prototype must precede invoking its Proxy trap.
        r#"(()=>{function C(){} let n=0;C.prototype=new Proxy({}, {getPrototypeOf(){n++;throw 9}});return Object.create(C.prototype) instanceof C && n===0})()"#,
        r#"(()=>{function C(){} let n=0;const marker={};let p=new Proxy({}, {getPrototypeOf(){n++;throw marker}});try{Object.create(p) instanceof C}catch(e){return e===marker&&n===1}return false})()"#,
        r#"(()=>{function C(){} let r=Proxy.revocable({},{});const o=Object.create(r.proxy);r.revoke();try{o instanceof C}catch(e){return e instanceof TypeError}return false})()"#,
        // Bound delegation, custom @@hasInstance and target getter order.
        r#"(()=>{let trace='';function C(){}const target=new Proxy(C,{get(t,k,r){if(k===Symbol.hasInstance)trace+='h';if(k==='prototype')trace+='p';return Reflect.get(t,k,r)}});let o=new C();let bound=target.bind(null);trace='';let a=o instanceof bound;if(!a||trace!=='hp')return false;Object.defineProperty(C,Symbol.hasInstance,{value(v){trace+='c';return v===o}});trace='';return o instanceof bound && trace==='hc'})()"#,
        // Nonobject candidate must not read target.prototype; object candidate
        // observes a getter throw before reading any candidate prototype.
        r#"(()=>{let trace='';const marker={};const c=new Proxy(function(){},{get(t,k,r){if(k===Symbol.hasInstance)return undefined;if(k==='prototype'){trace+='p';throw marker}return Reflect.get(t,k,r)}});const o=new Proxy({}, {getPrototypeOf(){trace+='o';return null}});if(1 instanceof c)return false;try{o instanceof c}catch(e){return e===marker&&trace==='p'}return false})()"#,
        r#"(()=>{function C(){}const a={},b=Object.create(a);try{Object.setPrototypeOf(a,b)}catch(e){return e instanceof TypeError && !(b instanceof C)}return false})()"#,
    ];
    for source in cases {
        assert_eq!(
            Runtime::new().new_context().eval(source).unwrap(),
            Value::Bool(true),
            "{source}"
        );
    }
}

#[test]
fn instanceof_chain_internal_cycle_returns_to_protocol() {
    use crate::engine::{heap::ObjectData, object::shape::Shape};
    let mut heap = Heap::new();
    let empty = heap.allocate_shape(Shape::new(None, []).unwrap()).unwrap();
    let object = heap
        .allocate_object(ObjectData::ordinary(empty, vec![]))
        .unwrap();
    let expected = heap
        .allocate_object(ObjectData::ordinary(empty, vec![]))
        .unwrap();
    let cyclic = heap
        .allocate_shape(Shape::new(Some(object), []).unwrap())
        .unwrap();
    heap.replace_object_layout(object, cyclic, vec![].into())
        .unwrap();
    assert_eq!(
        walk_ordinary_chain(&heap, object, expected).unwrap(),
        ChainWalk::Protocol(object)
    );
    heap.replace_object_layout(object, empty, vec![].into())
        .unwrap();
    heap.release_shape(cyclic).unwrap();
    heap.release_object(object).unwrap();
    heap.release_object(expected).unwrap();
    heap.release_shape(empty).unwrap();
}

#[test]
fn instanceof_chain_pending_cleanup_uses_existing_protocol() {
    let runtime = Runtime::new();
    let context = runtime.new_context();
    let expected = runtime.new_object(None).unwrap();
    let candidate = runtime.new_object(Some(&expected)).unwrap();
    let candidate_id = candidate.object_id();
    let resume = InstanceResume(Box::new(InstanceResumeState {
        pending_effect: InstanceStepPending::default(),
        realm: context.realm,
        candidate: JsValue::Object(candidate.into_handle()),
        target: expected.clone(),
        phase: Phase::Walk(expected),
    }));
    let discarded = runtime.new_object(None).unwrap();
    let state = runtime.0.state.borrow();
    drop(discarded);
    drop(state);
    assert!(runtime.0.deferred_references.has_pending());
    let step = resume.walk_ordinary(&runtime, candidate_id).unwrap();
    assert!(matches!(step, InstanceStep::Prototype { .. }));
    assert!(runtime.0.deferred_references.has_pending());
    assert!(matches!(
        finish(&runtime, context.realm, step).unwrap(),
        Completion::Return(JsValue::Bool(true))
    ));
    assert!(!runtime.0.deferred_references.has_pending());
}

#[test]
fn instanceof_chain_preserves_saturated_entry_and_prototype_retains() {
    use crate::engine::heap::RawId;
    for position in 0..3 {
        for count in [u32::MAX - 1, u32::MAX] {
            let runtime = Runtime::new();
            let context = runtime.new_context();
            let expected = runtime.new_object(None).unwrap();
            let middle = runtime.new_object(Some(&expected)).unwrap();
            let candidate = runtime.new_object(Some(&middle)).unwrap();
            let id = [
                candidate.object_id(),
                middle.object_id(),
                expected.object_id(),
            ][position];
            let ordinary_count = runtime
                .0
                .state
                .borrow()
                .heap
                .object_strong_count(id)
                .unwrap();
            let resume = InstanceResume(Box::new(InstanceResumeState {
                pending_effect: InstanceStepPending::default(),
                realm: context.realm,
                candidate: JsValue::Object(candidate.clone().into_handle()),
                target: runtime.new_object(None).unwrap(),
                phase: Phase::Walk(expected.clone()),
            }));
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(id), count);
            let result = resume
                .walk_ordinary(&runtime, candidate.object_id())
                .and_then(|step| finish(&runtime, context.realm, step));
            if count == u32::MAX {
                assert!(
                    matches!(result, Err(RuntimeError::Heap(HeapError::Overflow { .. }))),
                    "position {position}"
                );
            } else {
                assert!(
                    matches!(result, Ok(Completion::Return(JsValue::Bool(true)))),
                    "position {position}"
                );
            }
            assert_eq!(
                runtime.0.state.borrow().heap.object_strong_count(id),
                Ok(u32::MAX)
            );
            runtime
                .0
                .state
                .borrow_mut()
                .heap
                .set_strong_count_for_test(RawId::Object(id), ordinary_count);
        }
    }
}

#[test]
fn instanceof_chain_self_edge_preserves_two_temporary_retains() {
    use crate::engine::{
        heap::{ObjectData, RawId},
        object::shape::Shape,
    };
    let mut heap = Heap::new();
    let empty = heap.allocate_shape(Shape::new(None, []).unwrap()).unwrap();
    let object = heap
        .allocate_object(ObjectData::ordinary(empty, vec![]))
        .unwrap();
    let cyclic = heap
        .allocate_shape(Shape::new(Some(object), []).unwrap())
        .unwrap();
    heap.replace_object_layout(object, cyclic, vec![].into())
        .unwrap();
    let ordinary_count = heap.object_strong_count(object).unwrap();
    heap.set_strong_count_for_test(RawId::Object(object), u32::MAX - 2);
    // A self-match cannot complete before candidate and result roots are retained.
    assert_eq!(
        walk_ordinary_chain(&heap, object, object).unwrap(),
        ChainWalk::Protocol(object)
    );
    heap.retain_object(object).unwrap();
    heap.retain_object(object).unwrap();
    assert_eq!(heap.object_strong_count(object), Ok(u32::MAX));
    heap.release_object(object).unwrap();
    heap.release_object(object).unwrap();
    assert_eq!(heap.object_strong_count(object), Ok(u32::MAX));
    heap.set_strong_count_for_test(RawId::Object(object), ordinary_count);
    heap.replace_object_layout(object, empty, vec![].into())
        .unwrap();
    heap.release_shape(cyclic).unwrap();
    heap.release_object(object).unwrap();
    heap.release_shape(empty).unwrap();
}

#[test]
fn instanceof_chain_saturated_replies_transfer_without_a_new_retain() {
    use crate::engine::heap::RawId;
    for proxy in [false, true] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let Value::Object(object) = context
            .eval(if proxy { "new Proxy({}, {})" } else { "({})" })
            .unwrap()
        else {
            panic!("object result")
        };
        let expected = runtime.new_object(None).unwrap();
        let resume = InstanceResume(Box::new(InstanceResumeState {
            pending_effect: InstanceStepPending::default(),
            realm: context.realm,
            candidate: JsValue::Object(runtime.new_object(None).unwrap().into_handle()),
            target: runtime.new_object(None).unwrap(),
            phase: Phase::Walk(expected),
        }));
        let id = object.object_id();
        let ordinary_count = runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(id)
            .unwrap();
        let reply = object.clone();
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), u32::MAX);
        let pending = runtime.new_object(None).unwrap();
        let state = runtime.0.state.borrow();
        drop(pending);
        drop(state);
        let step = resume
            .prototype(&runtime, NativeConversion::Value(Some(reply)))
            .unwrap();
        let InstanceStep::Prototype { mut resume } = step else {
            panic!("reply must transfer to original protocol")
        };
        let receiver = resume.take_prototype_object();
        assert_eq!(receiver.object_id(), id);
        assert_eq!(
            runtime.0.state.borrow().heap.object_strong_count(id),
            Ok(u32::MAX)
        );
        assert!(runtime.0.deferred_references.has_pending());
        drop(receiver);
        drop(resume);
        runtime
            .0
            .state
            .borrow_mut()
            .heap
            .set_strong_count_for_test(RawId::Object(id), ordinary_count);
    }
}
