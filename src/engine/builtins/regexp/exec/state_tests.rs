//! Real native consumers plus direct owned-phase/failure witnesses.
use super::*;
use crate::engine::heap::{ObjectPayload, RawId, runtime::DeferredRefOp};

fn eval_true(source: &str) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
}
#[test]
fn regexp_state_all_primitive_inputs_and_symbol_error() {
    eval_true(
        r#"(()=>{ const values=[undefined,null,true,false,0,-0,NaN,Infinity,42,1.25,12n,123456789012345678901234567890n,'é','a'+'b'];
      for(const value of values){ const text=String(value), re=/./; if(re.exec(value)[0]!==text[0]||!re.test(value))return false; }
      for(const method of ['exec','test']){try{RegExp.prototype[method].call(/./,Symbol('x'));return false}catch(e){if(!(e instanceof TypeError))return false}}
      return true; })()"#,
    );
}
#[test]
fn regexp_state_exec_brand_precedes_input_but_test_gets_method_first() {
    eval_true(
        r#"(()=>{let trace=''; const input={toString(){trace+='i';return 'a'}};
      try{RegExp.prototype.exec.call({},input);return false}catch(e){if(!(e instanceof TypeError)||trace!=='')return false}
      const carrier={get exec(){trace+='g';return function(v){trace+='c';if(v!==input)throw 'input changed';return {}}}};
      return RegExp.prototype.test.call(carrier,input)&&trace==='gc';})()"#,
    );
}
#[test]
fn regexp_state_input_and_lastindex_compile_mutations_are_observed_after_both() {
    eval_true(
        r#"(()=>{let trace='',re=/a/g;
      const input={[Symbol.toPrimitive](hint){if(hint!=='string')throw hint;trace+='i';re.compile('b','g');re.lastIndex={[Symbol.toPrimitive](hint){if(hint!=='number')throw hint;trace+='l';re.compile('c','g');return 0}};return 'c'}};
      return re.exec(input)[0]==='c'&&re.lastIndex===1&&trace==='il';})()"#,
    );
}
#[test]
fn regexp_state_custom_exec_getter_bound_proxy_native_and_result_validation() {
    eval_true(
        r#"(()=>{let calls=0; const carrier={};
      Object.defineProperty(carrier,'exec',{get(){calls++;return new Proxy(function(v){calls++;if(this!==carrier||v!=='raw')throw 'roles';return null},{apply(t,r,a){calls++;return Reflect.apply(t,r,a)}})}});
      if(RegExp.prototype.test.call(carrier,'raw')||calls!==3)return false;
      const bound={exec:(function(v){if(this!==bound||v!==17)throw 'bound';return {}}).bind(null)};
      bound.exec=(function(v){if(this!==bound||v!==17)throw 'bound';return {}}).bind(bound);
      if(!RegExp.prototype.test.call(bound,17))return false;
      const actual=/a/; actual.exec=RegExp.prototype.exec; if(!RegExp.prototype.test.call(actual,'a'))return false;
      try{RegExp.prototype.test.call({exec(){return 1}},'x');return false}catch(e){return e instanceof TypeError}})()"#,
    );
}
#[test]
fn regexp_state_custom_exec_and_conversion_throws_preserve_identity() {
    eval_true(
        r#"(()=>{const marker={};
      for(const run of [()=>RegExp.prototype.test.call({get exec(){throw marker}},'x'),()=>RegExp.prototype.test.call({exec(){throw marker}},'x'),()=>/a/.exec({toString(){throw marker}})]){
        try{run();return false}catch(e){if(e!==marker)return false}}
      const re=/a/g;re.lastIndex={valueOf(){throw marker}};try{re.exec('a');return false}catch(e){return e===marker}})()"#,
    );
}
#[test]
fn regexp_state_noncallable_exec_fallback_and_nonregexp_receiver() {
    eval_true(
        r#"(()=>{const re=/a/;re.exec=7;if(!RegExp.prototype.test.call(re,'a'))return false;
      try{RegExp.prototype.test.call({exec:7},'a');return false}catch(e){return e instanceof TypeError}})()"#,
    );
}
#[test]
fn regexp_state_global_sticky_nonglobal_and_tolength_order() {
    eval_true(
        r#"(()=>{let n=0; const normal=/a/;normal.lastIndex={valueOf(){n++;return Infinity}};
      if(normal.exec('a')[0]!=='a'||n!==1||typeof normal.lastIndex!=='object')return false;
      const global=/a/g;global.lastIndex=Infinity;if(global.exec('a')!==null||global.lastIndex!==0)return false;
      global.lastIndex=-3;if(global.exec('a')[0]!=='a'||global.lastIndex!==1)return false;
      const sticky=/a/y;sticky.lastIndex=1;if(sticky.exec('ba')[0]!=='a'||sticky.lastIndex!==2)return false;
      sticky.lastIndex=0;if(sticky.exec('ba')!==null||sticky.lastIndex!==0)return false;
      const empty=/(?:)/g;empty.lastIndex=0;return empty.exec('a')[0]===''&&empty.lastIndex===0;})()"#,
    );
}
#[test]
fn regexp_state_readonly_lastindex_rejects_before_result_publication() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp)=context.eval("(()=>{const re=/a/g;Object.defineProperty(re,'lastIndex',{writable:false});return re})()").unwrap() else{panic!("RegExp")};
    let input = runtime
        .into_jsvalue(Value::String(JsString::from_static("a")))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.counts().object_nodes;
    let step = RegExpExecStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        RegExpNativeKind::Exec,
        &NativeInvocation::Call {
            this_value: JsValue::Object(regexp.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::from_raw(input.as_raw()).unwrap()],
        },
    )
    .unwrap();
    let RegExpExecStep::CyclePublished(Completion::Throw(error)) = step else {
        panic!("fresh strict error")
    };
    assert_eq!(
        state.heap.counts().object_nodes,
        before + 1,
        "no result/indices Array preceded the strict Error"
    );
    assert!(matches!(
        state
            .heap
            .object(match error {
                JsValue::Object(id) => id,
                _ => panic!("Error"),
            })
            .unwrap()
            .payload,
        ObjectPayload::Error
    ));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, error)
        .unwrap();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, input)
        .unwrap();
}
#[test]
fn regexp_state_named_indices_and_rope_input_keep_aliases() {
    eval_true(
        r#"(()=>{const text='a'+'b';const result=/(?<first>a)(?<last>b)?/d.exec(text);
      return result.input===text&&result.groups.first===result[1]&&result.indices.groups.first===result.indices[1]&&Object.getPrototypeOf(result.groups)===null&&Object.keys(result.groups).join(',')==='first,last';})()"#,
    );
}
#[test]
fn regexp_state_wait_abandonment_retires_real_input_and_receiver_roles() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp) = context.eval("/a/").unwrap() else {
        panic!("RegExp")
    };
    let input = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let re_before = state.heap.object_strong_count(regexp.object_id()).unwrap();
    let input_before = state.heap.object_strong_count(input.object_id()).unwrap();
    let step = RegExpExecStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        RegExpNativeKind::Exec,
        &NativeInvocation::Call {
            this_value: JsValue::Object(regexp.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::Object(input.object_id())],
        },
    )
    .unwrap();
    assert!(matches!(step, RegExpExecStep::Primitive { .. }));
    assert_eq!(
        state.heap.object_strong_count(input.object_id()),
        Ok(input_before + 2)
    );
    step.retire_in_state(&mut state, &runtime.0.poisoned)
        .unwrap();
    assert_eq!(
        state.heap.object_strong_count(input.object_id()),
        Ok(input_before)
    );
    assert_eq!(
        state.heap.object_strong_count(regexp.object_id()),
        Ok(re_before)
    );
}
#[test]
fn regexp_state_borrowed_branded_access_removes_only_temporary_header_max_failure() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp) = context.eval("/a/").unwrap() else {
        panic!("RegExp")
    };
    let input = runtime.new_object(None).unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let original = state.heap.object_strong_count(regexp.object_id()).unwrap();
    let RegExpExecStep::Primitive { mut resume } = RegExpExecStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        RegExpNativeKind::Exec,
        &NativeInvocation::Call {
            this_value: JsValue::Object(regexp.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 1,
            readable: vec![JsValue::Object(input.object_id())],
        },
    )
    .unwrap() else {
        panic!("input conversion")
    };
    let (conversion, _) = resume.take_primitive();
    state
        .release_owned_jsvalue(&runtime.0.poisoned, conversion)
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(regexp.object_id()), u32::MAX);
    let converted = JsValue::String(
        state
            .heap
            .allocate_string(JsString::from_static("a"))
            .unwrap(),
    );
    let RegExpExecStep::CyclePublished(Completion::Return(result)) = resume
        .resume_in_state(
            &mut state,
            &runtime.0.poisoned,
            Completion::Return(converted),
        )
        .unwrap()
    else {
        panic!("borrowed brand remains live")
    };
    assert!(!runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(regexp.object_id()),
        Ok(u32::MAX)
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(regexp.object_id()), original);
    state
        .release_owned_jsvalue(&runtime.0.poisoned, result)
        .unwrap();
}
#[test]
fn regexp_state_descriptor_output_second_retain_remains_recoverable() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp) = context.eval("/a/").unwrap() else {
        panic!("RegExp")
    };
    let last_index = runtime.new_object(None).unwrap();
    let key = runtime.pinned_property_key(PinnedAtom::LastIndex).unwrap();
    runtime
        .set_property_or_throw(
            context.realm,
            &regexp,
            &key,
            Value::Object(last_index.try_clone().unwrap()),
        )
        .unwrap();
    let input = JsValue::Int(7);
    let mut state = runtime.0.state.borrow_mut();
    let before = state
        .heap
        .object_strong_count(last_index.object_id())
        .unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(last_index.object_id()), u32::MAX - 1);
    let result = RegExpExecStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        RegExpNativeKind::Exec,
        &NativeInvocation::Call {
            this_value: JsValue::Object(regexp.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 1,
            readable: vec![input],
        },
    );
    assert!(result.is_err());
    assert!(!runtime.is_poisoned());
    assert_eq!(
        state.heap.object_strong_count(last_index.object_id()),
        Ok(u32::MAX)
    );
    state
        .heap
        .set_strong_count_for_test(RawId::Object(last_index.object_id()), before);
    drop(state);
    assert_eq!(context.eval("1+1").unwrap(), Value::Int(2));
}
#[test]
fn regexp_state_initial_real_receiver_retain_still_rejects_max() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp) = context.eval("/a/").unwrap() else {
        panic!("RegExp")
    };
    let mut state = runtime.0.state.borrow_mut();
    let before = state.heap.object_strong_count(regexp.object_id()).unwrap();
    state
        .heap
        .set_strong_count_for_test(RawId::Object(regexp.object_id()), u32::MAX);
    assert!(
        RegExpExecStep::start_in_state(
            &mut state,
            &runtime.0.poisoned,
            context.realm,
            RegExpNativeKind::Exec,
            &NativeInvocation::Call {
                this_value: JsValue::Object(regexp.object_id())
            },
            &NativeArguments {
                actual_arg_count: 1,
                readable: vec![JsValue::Int(1)]
            }
        )
        .is_err()
    );
    assert!(!runtime.is_poisoned());
    state
        .heap
        .set_strong_count_for_test(RawId::Object(regexp.object_id()), before);
}
#[test]
fn regexp_state_result_publication_quarantines_before_domain_suffix_cleanup() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp) = context.eval("/a/").unwrap() else {
        panic!("RegExp")
    };
    let input = runtime
        .into_jsvalue(Value::String(JsString::from_static("a")))
        .unwrap();
    let older = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let re_count = state.heap.object_strong_count(regexp.object_id()).unwrap();
    let nodes = state.heap.counts().object_nodes;
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
    let result = RegExpExecStep::start_in_state(
        &mut state,
        &runtime.0.poisoned,
        context.realm,
        RegExpNativeKind::Exec,
        &NativeInvocation::Call {
            this_value: JsValue::Object(regexp.object_id()),
        },
        &NativeArguments {
            actual_arg_count: 1,
            readable: vec![input],
        },
    );
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(
        state.heap.counts().object_nodes,
        nodes + 1,
        "fresh result published before older cleanup failed"
    );
    assert_eq!(
        state.heap.object_strong_count(regexp.object_id()),
        Ok(re_count + 1)
    );
    assert_eq!(state.heap.object_strong_count(later), Ok(0));
    assert!(state.heap.has_pending_zero_cleanup());
}
#[test]
fn regexp_state_test_retired_builtin_result_keeps_actual_publication_fact() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().unwrap();
    let Value::Object(regexp) = context.eval("/a/").unwrap() else {
        panic!("RegExp")
    };
    let input = runtime
        .into_jsvalue(Value::String(JsString::from_static("a")))
        .unwrap();
    let mut state = runtime.0.state.borrow_mut();
    let mut resume = RegExpExecResume::new(context.realm, true);
    resume.0.regexp = state
        .dup_jsvalue(&JsValue::Object(regexp.object_id()))
        .unwrap();
    resume.0.input = state.dup_jsvalue(&input).unwrap();
    let step = RegExpGuard::new(&mut state, &runtime.0.poisoned, resume)
        .builtin()
        .unwrap();
    assert!(matches!(
        step,
        RegExpExecStep::CyclePublished(Completion::Return(JsValue::Bool(true)))
    ));
    state
        .release_owned_jsvalue(&runtime.0.poisoned, input)
        .unwrap();
}
#[test]
fn regexp_state_unwind_quarantines_before_raw_resume_guard_traverses() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let original = runtime.new_object(None).unwrap().into_handle();
    let mut state = runtime.0.state.borrow_mut();
    let mut resume = RegExpExecResume::new(context.realm, false);
    resume.0.input = JsValue::Object(original);
    let before = state.heap.object_strong_count(original).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = RegExpGuard::new(&mut state, &runtime.0.poisoned, resume);
        panic!("RegExp phase fault");
    }));
    assert!(result.is_err());
    assert!(runtime.is_poisoned());
    assert_eq!(state.heap.object_strong_count(original), Ok(before));
}

#[test]
fn regexp_private_brand_error_keeps_fifo_at_actual_public_admission() {
    let runtime = Runtime::new();
    let context = runtime.new_context().unwrap();
    let stale = runtime.new_object(None).unwrap().into_handle();
    let later = runtime.new_object(None).unwrap().into_handle();
    runtime
        .0
        .state
        .borrow_mut()
        .release_jsvalue(JsValue::Object(stale))
        .unwrap();
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(stale));
    runtime
        .0
        .deferred_references
        .push_back(DeferredRefOp::Object(later));

    // The old nested native Error adapter drained this queue. The canonical
    // admitted phase keeps the real outer operation's coordinator boundary.
    let RegExpExecStep::CyclePublished(Completion::Throw(JsValue::Object(error))) =
        RegExpExecStep::start(
            &runtime,
            context.realm,
            RegExpNativeKind::Exec,
            &NativeInvocation::Call {
                this_value: JsValue::Null,
            },
            &NativeArguments {
                actual_arg_count: 1,
                readable: vec![JsValue::Int(1)],
            },
        )
        .unwrap()
    else {
        panic!("brand TypeError must remain local");
    };
    assert!(!runtime.is_poisoned());
    assert!(runtime.0.deferred_references.has_pending());
    runtime
        .0
        .state
        .borrow_mut()
        .release_owned_jsvalue(&runtime.0.poisoned, JsValue::Object(error))
        .unwrap();
    assert!(matches!(
        runtime.new_object(None),
        Err(RuntimeError::Heap(_))
    ));
    assert!(runtime.is_poisoned());
    assert!(runtime.0.deferred_references.has_pending());
    assert_eq!(
        runtime.0.state.borrow().heap.object_strong_count(later),
        Ok(1)
    );
}
