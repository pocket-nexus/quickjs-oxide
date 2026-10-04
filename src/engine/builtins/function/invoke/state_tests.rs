use super::*;
use crate::engine::{
    api::Value,
    code::function::metadata::ClosureVariableKind,
    heap::{HeapError, RawId},
};

fn evaluate(source: &str) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert!(!runtime.is_poisoned());
}
#[test]
fn invoke_state_four_selectors_forward_all_values_and_bound_chains() {
    evaluate(
        r#"(()=>{
        const o={},s=Symbol('s'),b=123456789012345678901234567890n;
        const values=[undefined,null,true,-7,-0,NaN,'ab'+String(9),s,b,o];
        function f(){return this===o && arguments.length===values.length && values.every((v,i)=>Object.is(v,arguments[i]));}
        const bound=f.bind(o,values[0],values[1]).bind({});
        function C(){this.values=Array.from(arguments);}
        return f.call(o,...values) && f.apply(o,values) && Reflect.apply(f,o,values)
            && Reflect.apply(bound,null,values.slice(2))
            && Reflect.construct(C,values).values.every((v,i)=>Object.is(v,values[i]));
    })()"#,
    );
}
#[test]
fn invoke_state_reflect_construct_keeps_explicit_newtarget_list_target_order() {
    evaluate(
        r#"(()=>{
        let log=''; const marker={};
        const list={get length(){log+='l';throw marker}};
        try {Reflect.construct(0,list,0)} catch(e) {if(e.name!=='TypeError'||log!=='')return false}
        try {Reflect.construct(0,list)} catch(e) {if(e!==marker||log!=='l')return false}
        log=''; function C(){};
        const target=new Proxy(C,{construct(t,a,n){log+='c';return Reflect.construct(t,a,n)}});
        const ok={get length(){log+='l';return 1},get 0(){log+='0';return 4}};
        function N(){}; const result=Reflect.construct(target,ok,N);
        return log==='l0c' && Object.getPrototypeOf(result)===N.prototype;
    })()"#,
    );
}
#[test]
fn invoke_state_apply_validates_target_before_list_and_nullish_exception_is_local() {
    evaluate(
        r#"(()=>{
        let hits=0; const list={get length(){hits++;throw 4}};
        for(const go of [()=>Function.prototype.apply.call(0,null,list),()=>Reflect.apply(0,null,list)]){
            try {go();return false} catch(e){if(e.name!=='TypeError'||hits!==0)return false}
        }
        function f(){return arguments.length};
        if(f.apply(null,null)!==0 || f.apply(null,undefined)!==0)return false;
        for(const v of [null,undefined]) {try{Reflect.apply(f,null,v);return false}catch(e){if(e.name!=='TypeError')return false}}
        return hits===0;
    })()"#,
    );
    evaluate(
        r#"(()=>{
        for (const target of [0,null,undefined,{},Symbol('x'),true,1n]) {
            try { target(...[]); return false; }
            catch (e) { if (!(e instanceof TypeError)) return false; }
        }
        const arrow=()=>4;
        try { new arrow(...[]); return false; }
        catch (e) { return e instanceof TypeError; }
    })()"#,
    );
}
#[test]
fn invoke_state_fixed_length_getters_proxy_and_abrupt_replies_use_one_order() {
    evaluate(
        r#"(()=>{
        let log='',calls=0; const marker={};
        function f(a,b){calls++;return a+b};
        const list={get length(){log+='l';return {valueOf(){log+='n';list[1]=8;return 2}}},get 0(){log+='0';return 2}};
        const p=new Proxy(list,{get(t,k,r){log+='p'+String(k);return Reflect.get(t,k,r)}});
        if(Reflect.apply(f,null,p)!==10 || log!=='plengthlnp00p1'||calls!==1)return false;
        log=''; const bad={get length(){log+='l';return 3},get 0(){log+='0';return 1},get 1(){log+='1';throw marker},get 2(){log+='2';return 2}};
        try {f.apply(null,bad);return false}catch(e){if(e!==marker)return false}
        return log==='l01' && calls===1;
    })()"#,
    );
}
#[test]
fn invoke_state_forwarded_function_call_chains_keep_real_native_backtrace_frames() {
    evaluate(
        r#"(()=>{
        let f=function(){throw new Error('deep')};
        for(let i=0;i<200;i++){const target=f;f=Function.prototype.call.bind(target,null)}
        try {f();return false} catch(e){return e.message==='deep' && typeof e.stack==='string' && e.stack.split('\n').length>100}
    })()"#,
    );
}
#[test]
fn invoke_state_host_panic_through_list_getter_and_forwarded_call_quarantines_raw_parents() {
    #[derive(Debug)]
    struct PanicClock(std::rc::Rc<Cell<bool>>);
    impl crate::engine::host::HostServices for PanicClock {
        fn now_millis(&self) -> i64 {
            assert!(!self.0.get(), "forwarded host panic");
            0
        }
        fn timezone_offset_minutes(&self, _: i64) -> i32 {
            0
        }
        fn random_seed(&self) -> u64 {
            1
        }
    }
    let panic = std::rc::Rc::new(Cell::new(false));
    let runtime = Runtime::new_with_host_services(PanicClock(panic.clone()));
    let mut context = runtime.new_context().unwrap();
    let owners = std::rc::Rc::strong_count(&runtime.0);
    panic.set(true);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        context.eval("Math.abs.apply(null,{get length(){return Date.now.call(null)}})")
    }));
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(std::rc::Rc::strong_count(&runtime.0), owners);
    assert_eq!(runtime.0.raw_execution_owners.get(), 0);
    assert!(context.eval("1").is_err());
}
#[test]
fn invoke_state_arguments_and_rest_preserve_mapped_aliases_unmapped_padding_and_cells() {
    evaluate(
        r#"(()=>{
        function mapped(a,b){const x=arguments; const capture=()=>a; a=7;x[1]=9;return x.length===2&&x[0]===7&&b===9&&capture()===7;}
        function unmapped(a,b){'use strict';const x=arguments;a=8;return x.length===1&&x[0]===3&&x[1]===undefined;}
        function defaults(a=1,b=2){const x=arguments;a=8;return x.length===1&&x[0]===3&&x[1]===undefined;}
        function rest(a,...r){const c=()=>a;a=6;return c()===6&&r.length===2&&r[0]===4&&r[1]===5;}
        return mapped(1,2)&&unmapped(3)&&defaults(3)&&rest(3,4,5);
    })()"#,
    );
}
#[test]
fn invoke_state_arguments_length_overflow_is_fresh_range_error_before_index_reads() {
    evaluate(
        r#"(()=>{let reads=0;const x={length:65535,get 0(){reads++;return 1}};try{Reflect.apply(()=>{},null,x);return false}catch(e){return e.name==='RangeError'&&reads===0&&e.message.includes('65534')}})()"#,
    );
}
#[test]
fn invoke_state_root_registration_rejection_retires_exact_raw_call_inputs() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context.eval("(function(){throw 99})").unwrap() else {
        panic!()
    };
    let callable = crate::engine::object::CallableRef::from_validated_object(function);
    let receiver = runtime.new_object(None).unwrap();
    let argument = runtime.new_object(None).unwrap();
    let id = callable.as_object().object_id();
    let before = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    let receiver_value = runtime
        .dup_jsvalue(&JsValue::Object(receiver.object_id()))
        .unwrap();
    let argv = vec![
        runtime
            .dup_jsvalue(&JsValue::Object(argument.object_id()))
            .unwrap(),
    ];
    runtime.0.raw_execution_owners.set(usize::MAX);
    let result = runtime.call_internal_jsvalue(context.realm, &callable, receiver_value, argv);
    runtime.0.raw_execution_owners.set(0);
    assert!(result.is_err());
    assert!(!runtime.is_poisoned());
    let state = runtime.0.state.borrow();
    assert_eq!(state.heap.object_strong_count(id).unwrap(), before);
    assert_eq!(
        state
            .heap
            .object_strong_count(receiver.object_id())
            .unwrap(),
        1
    );
    assert_eq!(
        state
            .heap
            .object_strong_count(argument.object_id())
            .unwrap(),
        1
    );
    assert!(state.active_frames.is_empty());
}
#[test]
fn invoke_state_fast_snapshot_checks_real_output_retain_and_rolls_back_prefix() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(array) = context.eval("[{},{}]").unwrap() else {
        panic!()
    };
    let (first, blocked) = {
        let state = runtime.0.state.borrow();
        let crate::engine::heap::ObjectPayload::Array {
            dense: Some(values),
        } = &state.heap.object(array.object_id()).unwrap().payload
        else {
            panic!()
        };
        let (crate::engine::heap::RawValue::Object(a), crate::engine::heap::RawValue::Object(b)) =
            (&values[0], &values[1])
        else {
            panic!()
        };
        (*a, *b)
    };
    let mut state = runtime.0.state.borrow_mut();
    let original = state.heap.object_strong_count(blocked).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(blocked), u32::MAX);
    let result = state.fast_array_like_values_jsvalue(&runtime.0.poisoned, array.object_id(), 2);
    let first_after = state.heap.object_strong_count(first).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(blocked), original);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(first_after, 1);
    assert!(!runtime.is_poisoned());
}
#[test]
fn invoke_state_call_suffix_overflow_retires_receiver_then_copied_prefix() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context.eval("(function(){throw 99})").unwrap() else {
        panic!()
    };
    let receiver = runtime.new_object(None).unwrap();
    let first = runtime.new_object(None).unwrap();
    let blocked = runtime.new_object(None).unwrap();
    let invocation = NativeInvocation::Call {
        this_value: JsValue::Object(function.object_id()),
    };
    let arguments = NativeArguments {
        actual_arg_count: 3,
        readable: vec![
            JsValue::Object(receiver.object_id()),
            JsValue::Object(first.object_id()),
            JsValue::Object(blocked.object_id()),
        ],
    };
    let mut state = runtime.0.state.borrow_mut();
    let target_count = state
        .heap
        .object_strong_count(function.object_id())
        .unwrap();
    let blocked_count = state.heap.object_strong_count(blocked.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(blocked.object_id()), u32::MAX);
    let result = InvokeStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        InvokeKind::Call,
        &invocation,
        &arguments,
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(blocked.object_id()), blocked_count);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(state.heap.object_strong_count(receiver.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(first.object_id()), Ok(1));
    assert_eq!(
        state.heap.object_strong_count(function.object_id()),
        Ok(target_count)
    );
    assert!(!runtime.is_poisoned());
}
#[test]
fn invoke_state_call_fatal_receiver_rollback_preserves_copied_argv_and_target() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context.eval("(function(){throw 99})").unwrap() else {
        panic!()
    };
    let receiver = runtime.new_object(None).unwrap();
    let first = runtime.new_object(None).unwrap();
    let blocked = runtime.new_object(None).unwrap();
    let older = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let invocation = NativeInvocation::Call {
        this_value: JsValue::Object(function.object_id()),
    };
    let arguments = NativeArguments {
        actual_arg_count: 3,
        readable: vec![
            JsValue::Object(receiver.object_id()),
            JsValue::Object(first.object_id()),
            JsValue::Object(blocked.object_id()),
        ],
    };
    let mut state = runtime.0.state.borrow_mut();
    let target_count = state
        .heap
        .object_strong_count(function.object_id())
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(older))
        .unwrap();
    state
        .heap
        .queue_release_for_test(RawId::Object(later))
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(older), 1);
    state
        .heap
        .set_strong_count_for_test(RawId::Object(blocked.object_id()), u32::MAX);
    assert!(matches!(
        InvokeStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            InvokeKind::Call,
            &invocation,
            &arguments,
        ),
        Err(RuntimeError::Poisoned)
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(receiver.object_id()), Ok(1));
    assert_eq!(state.heap.object_strong_count(first.object_id()), Ok(2));
    assert_eq!(
        state.heap.object_strong_count(function.object_id()),
        Ok(target_count + 1)
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}
#[test]
fn invoke_state_mapped_public_prefix_prioritizes_callee_retain_before_foreign_cell() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context.eval("(function(){})").unwrap() else {
        panic!()
    };
    let other = Runtime::new();
    let cell = other
        .new_var_ref(JsValue::Int(1), false, false, ClosureVariableKind::Normal)
        .unwrap();
    let input = cell.try_clone().unwrap();
    let id = function.object_id();
    let saved = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), u32::MAX);
    let result = runtime.new_mapped_arguments_object(context.realm, &function, vec![input]);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), saved);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert_eq!(
        other
            .0
            .state
            .borrow()
            .heap
            .var_ref_strong_count(cell.id())
            .unwrap(),
        1
    );
    assert!(!runtime.is_poisoned());
}
#[test]
fn invoke_state_mapped_public_pending_admission_precedes_callee_retain() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context.eval("(function(){})").unwrap() else {
        panic!()
    };
    let pending = runtime.new_object(None).unwrap();
    let pending_id = pending.object_id();
    {
        let state = runtime.0.state.borrow();
        drop(pending);
        drop(state);
    }
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(pending_id), 0);
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(function.object_id()), u32::MAX);
    let result = runtime.new_mapped_arguments_object(context.realm, &function, Vec::new());
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(
        runtime
            .0
            .state
            .borrow()
            .heap
            .object_strong_count(function.object_id())
            .unwrap(),
        u32::MAX
    );
}
#[test]
fn invoke_state_mapped_publication_quarantines_cell_producers_before_guard_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context.eval("(function(){})").unwrap() else {
        panic!()
    };
    let warm = runtime
        .new_var_ref(JsValue::Int(1), false, false, ClosureVariableKind::Normal)
        .unwrap();
    let _layout = runtime
        .new_mapped_arguments_object(context.realm, &function, vec![warm])
        .unwrap();
    let first = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let proof = state
        .checked_arguments_callee(&runtime.0.poisoned, function.object_id())
        .unwrap();
    let cell = state
        .new_var_ref(
            &runtime.0.poisoned,
            JsValue::Int(2),
            false,
            false,
            ClosureVariableKind::Normal,
        )
        .unwrap();
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
    let result =
        state.new_mapped_arguments_object(&runtime.0.poisoned, context.realm, proof, vec![cell]);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Invariant(
            "finalization count disagrees with its queue/cycle state"
        )))
    ));
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.var_ref_strong_count(cell).unwrap(), 2);
    assert_eq!(state.heap.object_strong_count(later).unwrap(), 0);
    assert!(state.heap.has_pending_zero_cleanup());
}

#[test]
fn invoke_state_public_finish_preserves_typed_callback_retain_failure() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(function) = context
        .eval("var invokeLateFailureRan=false; (function(){invokeLateFailureRan=true})")
        .unwrap()
    else {
        panic!("function result")
    };
    let receiver = runtime.new_object(None).unwrap();
    let argument = runtime.new_object(None).unwrap();
    let id = function.object_id();
    let original = runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(id)
        .unwrap();
    let step = InvokeStep::Call(Box::new(InvokeCall {
        target: InvokeCallTarget::Callable(id),
        receiver: runtime
            .dup_jsvalue(&JsValue::Object(receiver.object_id()))
            .unwrap(),
        arguments: vec![
            runtime
                .dup_jsvalue(&JsValue::Object(argument.object_id()))
                .unwrap(),
        ],
    }));
    {
        let mut state = runtime.0.state.borrow_mut();
        state.heap.retain_object(id).unwrap();
        state
            .heap
            .set_strong_count_for_test(RawId::Object(id), u32::MAX);
    }
    let result = finish(&runtime, context.realm, step);
    // MAX uses the existing immortal release sentinel. Restore the count of
    // the remaining public root after the raw call request has retired.
    runtime
        .0
        .state
        .borrow_mut()
        .heap
        .set_strong_count_for_test(RawId::Object(id), original);
    assert!(matches!(
        result,
        Err(RuntimeError::Heap(HeapError::Overflow { .. }))
    ));
    assert!(!runtime.is_poisoned());
    {
        let state = runtime.0.state.borrow();
        assert_eq!(
            state
                .heap
                .object_strong_count(receiver.object_id())
                .unwrap(),
            1
        );
        assert_eq!(
            state
                .heap
                .object_strong_count(argument.object_id())
                .unwrap(),
            1
        );
        assert!(state.active_frames.is_empty());
    }
    assert_eq!(
        context.eval("invokeLateFailureRan").unwrap(),
        Value::Bool(false)
    );
}
