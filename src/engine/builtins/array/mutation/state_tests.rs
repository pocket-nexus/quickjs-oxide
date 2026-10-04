//! Contract witnesses for the complete raw mutation family; source-prepared.
use super::*;
use crate::engine::atom::AtomIdx;
use crate::engine::heap::{PropertySlot, RawId, RawValue};
use crate::engine::object::shape::PropertyFlags;

fn eval(source: &str) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
}
#[test]
fn all_four_actual_vm_selectors_preserve_all_value_kinds_and_alias_results() {
    eval(
        r#"(()=>{
 const o={},s=Symbol('value'),v=[undefined,null,true,3,-0,'leaf',123456789012345678901234567890n,s,o];
 for(const x of v){const a=[];if(a.push(x)!==1||!Object.is(a.pop(),x)||a.length!==0)return false;
 const b=[9];if(b.unshift(x)!==2||!Object.is(b.shift(),x)||b[0]!==9)return false;}return true;
})()"#,
    );
}
#[test]
fn holes_and_inherited_last_getter_are_read_before_pop_delete_and_length() {
    eval(
        r#"(()=>{
 let log='';const p={get 2(){log+='g';return 17;}};const a=Object.create(p);a.length=3;
 if(Array.prototype.pop.call(a)!==17||a.length!==2||log!=='g'||Object.hasOwn(a,'2'))return false;
 const b=[,,];return b.pop()===undefined&&b.length===1&&!(0 in b);
})()"#,
    );
}
#[test]
fn length_conversion_snapshot_is_kept_when_callbacks_change_actual_length() {
    eval(
        r#"(()=>{
 let log='';const a={get length(){log+='l';return {valueOf(){log+='n';return 2;}}},set length(v){log+='L'+v;},get 1(){log+='g';this.extra=4;return 8;}};
 return Array.prototype.pop.call(a)===8&&log==='lngL1'&&a.extra===4;
})()"#,
    );
}
#[test]
fn selected_getter_setter_and_proxy_traps_run_once_in_exact_order() {
    eval(
        r#"(()=>{
 let log='';const t={length:1,0:6};const p=new Proxy(t,{get(o,k,r){log+='g'+k+';';return Reflect.get(o,k,r)},set(o,k,v,r){log+='s'+k+';';return Reflect.set(o,k,v,r)},deleteProperty(o,k){log+='d'+k+';';return Reflect.deleteProperty(o,k)}});
 if(Array.prototype.pop.call(p)!==6||log!=='glength;g0;d0;slength;')return false;
 log='';return Array.prototype.push.call(p,8)===1&&log==='glength;s0;slength;'&&t[0]===8;
})()"#,
    );
}
#[test]
fn shift_and_unshift_resume_the_selected_copy_phase_without_restarting_length() {
    eval(
        r#"(()=>{
 let n=0;const t={0:'a',2:'c',get length(){n++;return 3},set length(v){this.final=v}};
 if(Array.prototype.shift.call(t)!=='a'||n!==1||t[1]!=='c'||Object.hasOwn(t,'0')||t.final!==2)return false;
 let reads=0;const u={0:4,1:5,get length(){reads++;return 2},set length(v){this.final=v}};
 return Array.prototype.unshift.call(u,1,2)===4&&reads===1&&u[0]===1&&u[1]===2&&u[2]===4&&u[3]===5&&u.final===4;
})()"#,
    );
}
#[test]
fn partial_writes_and_delete_rejection_preserve_the_original_failure_order() {
    eval(
        r#"(()=>{
 const a={length:0};Object.defineProperty(a,'length',{writable:false});let caught=false;
 try{Array.prototype.push.call(a,7,8)}catch(e){caught=e instanceof TypeError}
 if(!caught||a[0]!==7||a[1]!==8||a.length!==0)return false;
 const b={length:1};Object.defineProperty(b,'0',{value:5,configurable:false});caught=false;
 try{Array.prototype.pop.call(b)}catch(e){caught=e instanceof TypeError}
 return caught&&b[0]===5&&b.length===1;
})()"#,
    );
}
#[test]
fn empty_pop_still_sets_length_and_push_checks_safe_limit_before_first_write() {
    eval(
        r#"(()=>{
 let log='';const a={get length(){log+='g';return -5},set length(v){log+='s'+v}};
 if(Array.prototype.pop.call(a)!==undefined||log!=='gs0')return false;
 const b={length:9007199254740991};let caught=false;try{Array.prototype.push.call(b,1)}catch(e){caught=e instanceof TypeError}
 return caught&&b.length===9007199254740991&&!Object.hasOwn(b,'9007199254740991');
})()"#,
    );
}
#[test]
fn string_virtual_and_typed_index_rejections_keep_language_semantics() {
    eval(
        r#"(()=>{
 let caught=0;try{Array.prototype.pop.call('ab')}catch(e){if(e instanceof TypeError)caught++}
 const a=new Int32Array([3,4]);try{Array.prototype.pop.call(a)}catch(e){if(e instanceof TypeError)caught++}
 try{Array.prototype.push.call(a,9)}catch(e){if(e instanceof TypeError)caught++}
 return caught===3&&a.length===2&&a[0]===3&&a[1]===4;
})()"#,
    );
}
#[test]
fn mapped_arguments_detach_delete_but_keep_unremoved_aliases() {
    eval(
        r#"(function(a,b){
 if(Array.prototype.pop.call(arguments)!==b||arguments.length!==1||Object.hasOwn(arguments,'1'))return false;
 a=11;if(arguments[0]!==11)return false;return Array.prototype.push.call(arguments,12)===2&&arguments[1]===12&&b===2;
})(1,2)"#,
    );
}
#[test]
fn nested_getter_mutation_restores_parent_and_lower_operand_owners() {
    eval(
        r#"(()=>{
 const marker={},a={get length(){const b=[4,5];if(b.shift()!==4||b.unshift(8)!==2)throw 1;return 1},get 0(){return marker},set length(v){this.n=v}};
 return [marker,Array.prototype.pop.call(a),marker].every(x=>x===marker)&&a.n===0;
})()"#,
    );
}
#[test]
fn getter_throw_catch_preserves_fault_owner_and_aborts_delete_length() {
    eval(
        r#"(()=>{
 const marker={},a={length:1,get 0(){throw marker}};let actual;
 try{Array.prototype.pop.call(a)}catch(e){actual=e}
 return actual===marker&&a.length===1&&Object.hasOwn(a,'0');
})()"#,
    );
}
#[test]
fn pop_ignored_arguments_keep_existing_checked_snapshot_role() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap();
    let child = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), u32::MAX);
    let result = MutationStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        MutationKind::Pop(ArrayPopKind::Shift),
        &NativeInvocation::Call {
            this_value: JsValue::Object(target.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::Object(child.object_id())],
        },
    );
    assert!(result.is_err());
    assert!(!runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(child.object_id()),
        Ok(u32::MAX)
    );
    assert_eq!(state.heap.object_strong_count(target.object_id()), Ok(1));
    state
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), 1);
}
#[test]
fn two_original_checked_read_temporaries_remain_required_at_max() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(target.object_id()), u32::MAX - 2);
    let step = MutationStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        MutationKind::Push(ArrayPushKind::Unshift),
        &NativeInvocation::Call {
            this_value: JsValue::Object(target.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 0,
            readable: vec![],
        },
    )
    .unwrap();
    let MutationStep::Read { mut resume } = step else {
        panic!("selected length read")
    };
    assert!(
        resume
            .select_read_in_state(&mut state, &runtime.0.poisoned, runtime.domain_id())
            .is_err()
    );
    assert!(!runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(target.object_id()),
        Ok(u32::MAX)
    );
    resume
        .retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    // Successful checked copies reached the existing immortal saturation
    // sentinel before the next copy was rejected. Release preserves MAX.
    assert_eq!(
        state.heap.object_strong_count(target.object_id()),
        Ok(u32::MAX)
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(target.object_id()), 1);
}
#[test]
fn dense_store_needed_owner_overflow_is_recoverable_and_inert() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let target = runtime.new_array(context.realm).unwrap();
    let child = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), u32::MAX);
    assert!(
        state
            .try_dense_push(
                &runtime.0.poisoned,
                target.object_id(),
                &JsValue::Object(child.object_id())
            )
            .is_err()
    );
    assert!(!runtime.is_poisoned());
    assert_eq!(state.heap.array_dense_len(target.object_id()), Ok(Some(0)));
    assert_eq!(
        state.heap.object_strong_count(child.object_id()),
        Ok(u32::MAX)
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(child.object_id()), 1);
}
#[test]
fn destructive_dense_pop_failure_quarantines_before_length_publication() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let target = runtime.new_array(context.realm).unwrap();
    runtime.try_dense_push(&target, &JsValue::Int(8)).unwrap();
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
    assert!(
        state
            .try_dense_pop(&runtime.0.poisoned, target.object_id())
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.array_dense_len(target.object_id()), Ok(Some(0)));
    assert!(matches!(
        state.heap.object(target.object_id()).unwrap().slots[0],
        PropertySlot::Data(RawValue::Int(1))
    ));
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}
#[test]
fn finite_domain_retirement_stops_before_argument_suffix_on_first_fatal() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let source = runtime.new_object(None).unwrap().into_handle();
    let bad = runtime.new_object(None).unwrap().into_handle();
    let suffix = runtime
        .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
            "unretired suffix",
        )))
        .unwrap();
    let JsValue::String(suffix_id) = suffix else {
        panic!("leaf")
    };
    let mut resume = MutationStep::owner(
        context.realm,
        MutationKind::Push(ArrayPushKind::Unshift),
        source,
    );
    resume.0.arguments = vec![JsValue::Object(bad), JsValue::String(suffix_id)];
    let mut state = runtime.0.state.borrow_mut();
    state.heap.set_strong_count_for_test(RawId::Object(bad), 0);
    assert!(
        resume
            .retire_in_state(&mut state, &runtime.0.poisoned)
            .is_err()
    );
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.strong_count(RawId::String(suffix_id)), Ok(1));
    assert_eq!(state.heap.object_strong_count(source), Ok(1));
}
#[test]
fn legacy_start_preserves_this_overflow_before_pending_fifo_admission() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let target = runtime.new_object(None).unwrap();
    let pending = runtime.new_object(None).unwrap();
    let pending_id = pending.object_id();
    {
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(target.object_id()), u32::MAX);
        drop(pending);
    }
    assert!(runtime.0.deferred_references.has_pending());
    let result = MutationStep::start(
        &runtime,
        context.realm,
        MutationKind::Push(ArrayPushKind::Unshift),
        &NativeInvocation::Call {
            this_value: JsValue::Object(target.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 0,
            readable: vec![],
        },
    );
    assert!(result.is_err());
    assert!(!runtime.is_poisoned());
    assert!(runtime.0.deferred_references.has_pending());
    let mut state = runtime.0.state.borrow_mut();
    assert!(state.heap.object(pending_id).is_ok());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(target.object_id()), 1);
}
#[test]
fn legacy_start_preserves_prototype_overflow_before_selected_allocation_admission() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let input = runtime
        .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
            "boxed Array receiver",
        )))
        .unwrap();
    let prototype = runtime
        .0
        .state
        .borrow()
        .primitive_prototype_id_for_realm(
            context.realm,
            crate::engine::builtins::native::PrimitiveKind::String,
        )
        .unwrap();
    let saved = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(prototype)
        .unwrap();
    let pending = runtime.new_object(None).unwrap();
    {
        let mut state = runtime.0.state.borrow_mut();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(prototype), u32::MAX);
        drop(pending);
    }
    let invocation = NativeInvocation::Call { this_value: input };
    let result = MutationStep::start(
        &runtime,
        context.realm,
        MutationKind::Push(ArrayPushKind::Unshift),
        &invocation,
        &NativeArguments {
            actual_arg_count: 0,
            readable: vec![],
        },
    );
    assert!(result.is_err());
    assert!(!runtime.is_poisoned());
    assert!(runtime.0.deferred_references.has_pending());
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(prototype), saved);
    invocation.release(&runtime).unwrap();
}
#[test]
fn boxed_receiver_fact_and_domain_are_live_without_any_runtime_owner_clone() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let input = runtime
        .into_jsvalue(Value::String(crate::engine::value::JsString::from_static(
            "ab",
        )))
        .unwrap();
    let count = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    let invocation = NativeInvocation::Call { this_value: input };
    let step = MutationStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        MutationKind::Pop(ArrayPopKind::Pop),
        &invocation,
        &NativeArguments {
            actual_arg_count: 0,
            readable: vec![],
        },
    )
    .unwrap();
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), count);
    let MutationStep::CyclePublishedRead { resume } = step else {
        panic!("actual boxed producer fact")
    };
    let object = resume.0.object.unwrap();
    assert!(state.heap.object(object).is_ok());
    runtime.0.gc_pressure.remaining.set(0);
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .unwrap();
    assert!(state.heap.object(object).is_ok());
    resume
        .retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    invocation
        .release_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
}

fn install_index_zero_autoinit(
    state: &mut RuntimeState,
    poisoned: &Cell<bool>,
    realm: ContextId,
    object: ObjectId,
) {
    let key = state.property_key_atom_for_index(0).unwrap();
    state
        .define_native_builtin_auto_init_with_key(
            poisoned,
            object,
            realm,
            key,
            NativeFunctionId::MathRandom,
            "random",
            0,
            0,
            PropertyFlags::data(true, false, true),
        )
        .unwrap();
    state.atoms.release(key).unwrap();
}

fn own_data(state: &RuntimeState, object: ObjectId, key: Atom) -> Option<RawValue> {
    let object = state.heap.object(object).unwrap();
    let index = state
        .heap
        .shape(object.shape)
        .unwrap()
        .find(AtomIdx::from_raw(key.raw()))?;
    let PropertySlot::Data(value) = &object.slots[index as usize] else {
        panic!("expected ordinary data property")
    };
    Some(value.clone())
}

#[test]
fn actual_autoinit_set_fact_continues_to_length_in_public_finisher() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(target) = context.eval("({length:0})").unwrap() else {
        panic!("ordinary Array receiver")
    };
    install_index_zero_autoinit(
        &mut runtime.0.state.borrow_mut(),
        &runtime.0.poisoned,
        context.realm,
        target.object_id(),
    );
    let step = MutationStep::start(
        &runtime,
        context.realm,
        MutationKind::Push(ArrayPushKind::Push),
        &NativeInvocation::Call {
            this_value: JsValue::Object(target.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::Int(7)],
        },
    )
    .unwrap();
    assert!(matches!(
        finish(&runtime, context.realm, step).unwrap(),
        Completion::Return(JsValue::Int(1))
    ));
    let mut state = runtime.0.state.borrow_mut();
    let index = state.property_key_atom_for_index(0).unwrap();
    let length = state.pinned_atoms.get(PinnedAtom::Length);
    assert!(matches!(
        own_data(&state, target.object_id(), index),
        Some(RawValue::Int(7))
    ));
    assert!(matches!(
        own_data(&state, target.object_id(), length),
        Some(RawValue::Int(1))
    ));
    state.atoms.release(index).unwrap();
    assert_eq!(state.heap.object_strong_count(target.object_id()), Ok(1));
    assert!(!runtime.is_poisoned());
}

#[test]
fn first_autoinit_fact_arms_next_set_before_more_writes_and_collects_with_raw_owners() {
    use crate::engine::api::GcPolicy;
    use crate::engine::value::conversion::number::NumberStep;

    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let Value::Object(target) = context
        .eval("(()=>{let dead={};dead.self=dead;return {length:0}})()")
        .unwrap()
    else {
        panic!("ordinary receiver plus unreachable cycle")
    };
    let target = target.into_handle();
    let values = [
        runtime.new_object(None).unwrap().into_handle(),
        runtime.new_object(None).unwrap().into_handle(),
        runtime.new_object(None).unwrap().into_handle(),
    ];
    let rc = std::rc::Rc::strong_count(&runtime.0);
    let mut state = runtime.0.state.borrow_mut();
    install_index_zero_autoinit(&mut state, &runtime.0.poisoned, context.realm, target);
    let mut arguments = NativeArguments {
        actual_arg_count: values.len(),
        readable: values.into_iter().map(JsValue::Object).collect(),
    };
    let step = MutationStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        MutationKind::Push(ArrayPushKind::Push),
        &NativeInvocation::Call {
            this_value: JsValue::Object(target),
        },
        &arguments,
    )
    .unwrap();
    // Only the production raw Array domain owns the receiver and argv now.
    state
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(target))
        .unwrap();
    for value in &mut arguments.readable {
        state
            .release_owned_jsvalue(
                &runtime.0.poisoned,
                std::mem::replace(value, JsValue::Undefined),
            )
            .unwrap();
    }
    let MutationStep::Read { mut resume } = step else {
        panic!("actual length read")
    };
    let ReadStep::Ready(read) = resume
        .select_read_in_state(&mut state, &runtime.0.poisoned, runtime.domain_id())
        .unwrap()
    else {
        panic!("ordinary length selection")
    };
    resume
        .finish_read_selection_in_state(&mut state, &runtime.0.poisoned, &read)
        .unwrap();
    let OwnedRead::Complete(Some(value)) = read else {
        panic!("actual length result")
    };
    let MutationStep::Number { value, resume } = resume
        .resume_in_state(&mut state, &runtime.0.poisoned, Completion::Return(value))
        .unwrap()
    else {
        panic!("length conversion")
    };
    let NumberStep::Complete(number) =
        NumberStep::start_jsvalue_in_state(&mut state, &runtime.0.poisoned, context.realm, value)
            .unwrap()
    else {
        panic!("ordinary numeric length")
    };
    let step = resume
        .number_in_state(&mut state, &runtime.0.poisoned, number)
        .unwrap();
    let MutationStep::CyclePublishedSet { progress, resume } = &step else {
        panic!("real AutoInit must checkpoint at the next selected Set")
    };
    assert!(matches!(
        progress.as_ref(),
        SetProgress::Complete(SetAction::Complete)
    ));
    assert!(matches!(resume.0.phase, Phase::Write));
    assert_eq!(resume.0.cursor, 1, "next reply has not been consumed");
    let index0 = state.property_key_atom_for_index(0).unwrap();
    let index1 = state.property_key_atom_for_index(1).unwrap();
    let index2 = state.property_key_atom_for_index(2).unwrap();
    let length = state.pinned_atoms.get(PinnedAtom::Length);
    assert_eq!(resume.key(), index1);
    assert!(
        matches!(own_data(&state, target, index0), Some(RawValue::Object(id)) if id == values[0])
    );
    assert!(
        matches!(own_data(&state, target, index1), Some(RawValue::Object(id)) if id == values[1])
    );
    assert!(own_data(&state, target, index2).is_none());
    assert!(matches!(
        own_data(&state, target, length),
        Some(RawValue::Int(0))
    ));
    let owners = values.map(|id| state.heap.object_strong_count(id).unwrap());
    assert_eq!(owners, [2, 2, 1]);
    assert_eq!(state.heap.object_strong_count(target), Ok(1));
    let before = state.heap.counts().object_nodes;
    runtime.0.gc_pressure.policy.set(GcPolicy::Automatic);
    runtime.0.gc_pressure.remaining.set(0);
    state
        .collect_if_requested(&runtime.0.gc_pressure, &runtime.0.poisoned)
        .unwrap();
    assert!(state.heap.counts().object_nodes < before);
    assert_eq!(
        values.map(|id| state.heap.object_strong_count(id).unwrap()),
        owners
    );
    assert_eq!(state.heap.object_strong_count(target), Ok(1));
    assert!(own_data(&state, target, index2).is_none());
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), rc);
    let MutationStep::CyclePublishedSet { progress, resume } = step else {
        unreachable!()
    };
    let SetProgress::Complete(action) = *progress else {
        unreachable!()
    };
    let completed = resume
        .set_in_state(&mut state, &runtime.0.poisoned, action)
        .unwrap();
    assert!(matches!(
        completed,
        MutationStep::Complete(Completion::Return(JsValue::Int(3)))
    ));
    completed
        .retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    assert!(state.heap.object(target).is_err());
    for value in values {
        assert!(state.heap.object(value).is_err());
    }
    for key in [index0, index1, index2] {
        state.atoms.release(key).unwrap();
    }
    assert!(!runtime.is_poisoned());
    assert!(!runtime.0.deferred_references.has_pending());
}
