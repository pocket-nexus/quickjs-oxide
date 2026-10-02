use super::*;

fn assert_consumed(resume: &RegExpSplitResume, address: &mut Option<usize>) {
    let current = &*resume.0 as *const RegExpSplitResumeState as usize;
    assert_eq!(*address.get_or_insert(current), current);
    let pending = &resume.0.step_pending;
    assert!(pending.value.is_none() && pending.hint.is_none());
    assert!(pending.object.is_none() && pending.key.is_none());
    assert!(pending.regexp.is_none() && pending.constructor.is_none());
    assert!(pending.arguments.is_none());
    assert!(pending.exec_regexp.is_none() && pending.input.is_none());
}

// Use the real synchronous request consumers while observing the domain's
// allocation identity. In particular, do not synthesize exec or capture replies.
fn drive_observing(
    runtime: &Runtime,
    realm: ContextId,
    mut step: RegExpSplitStep,
) -> (Completion, usize) {
    let mut address = None;
    let mut captures = 0;
    loop {
        step = match step {
            RegExpSplitStep::Complete(result) => return (result, captures),
            RegExpSplitStep::Primitive { mut resume } => {
                let value = resume.take_primitive_value();
                let hint = resume.take_primitive_hint();
                assert_consumed(&resume, &mut address);
                let result = if matches!(value, JsValue::Object(_)) {
                    runtime.to_primitive_jsvalue(realm, value, hint).unwrap()
                } else {
                    Completion::Return(value)
                };
                resume.resume(runtime, result).unwrap()
            }
            RegExpSplitStep::Read { mut resume } => {
                captures += usize::from(matches!(resume.0.phase, Phase::Capture { .. }));
                let object = resume.take_read_object();
                let key = resume.take_read_key();
                assert_consumed(&resume, &mut address);
                let result = runtime
                    .internal_get_jsvalue(
                        realm,
                        &object,
                        &key,
                        JsValue::Object(object.clone().into_handle()),
                    )
                    .unwrap();
                resume.resume(runtime, result).unwrap()
            }
            RegExpSplitStep::Species { mut resume } => {
                let regexp = resume.take_species_regexp();
                assert_consumed(&resume, &mut address);
                let result = runtime.regexp_species_constructor(realm, &regexp).unwrap();
                resume.species(runtime, result).unwrap()
            }
            RegExpSplitStep::Construct { mut resume } => {
                let constructor = resume.take_construct_constructor();
                let arguments = resume.take_construct_arguments();
                assert_consumed(&resume, &mut address);
                let result = runtime
                    .construct_internal_jsvalue(
                        realm,
                        &constructor,
                        crate::engine::vm::call::ConstructNewTarget::Validated(constructor.clone()),
                        arguments,
                    )
                    .unwrap();
                resume.resume(runtime, result).unwrap()
            }
            RegExpSplitStep::Set { mut resume } => {
                let object = resume.take_set_object();
                let key = resume.take_set_key();
                let value = resume.take_set_value();
                assert_consumed(&resume, &mut address);
                let result = runtime
                    .internal_set_jsvalue(
                        realm,
                        &object,
                        &key,
                        value,
                        JsValue::Object(object.clone().into_handle()),
                    )
                    .unwrap();
                resume.set(runtime, result).unwrap()
            }
            RegExpSplitStep::Exec { mut resume } => {
                let regexp = resume.take_exec_regexp();
                let input = resume.take_exec_input();
                assert_consumed(&resume, &mut address);
                let result = runtime.regexp_exec_abstract(realm, regexp, input).unwrap();
                resume.resume(runtime, result).unwrap()
            }
        };
    }
}

#[test]
fn regexp_split_resident_all_requests_share_one_box_and_consume_their_fields() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let invocation = NativeInvocation::Call {
        this_value: runtime
            .into_jsvalue(
                context
                    .eval(
                        r#"(()=>{
                const re=/(-)|(\+)/;
                re.constructor={[Symbol.species]:function(pattern,flags){
                    return new RegExp(pattern,flags);
                }};
                return re;
            })()"#,
                    )
                    .unwrap(),
            )
            .unwrap(),
    };
    let arguments = NativeArguments {
        actual_arg_count: 1,
        readable: vec![
            runtime
                .into_jsvalue(Value::String(JsString::from_static("a-b+c")))
                .unwrap(),
            JsValue::Undefined,
        ],
    };
    let step = RegExpSplitStep::start(&runtime, context.realm, &invocation, &arguments).unwrap();
    let (Completion::Return(value), captures) = drive_observing(&runtime, context.realm, step)
    else {
        panic!("split unexpectedly threw");
    };
    assert_eq!(captures, 4);
    let Value::Object(result) = runtime.root_and_release_jsvalue(value).unwrap() else {
        panic!("split did not return an Array");
    };
    let length_key = runtime.intern_property_key("length").unwrap();
    assert_eq!(
        context.get_property(&result, &length_key).unwrap(),
        Value::Int(7)
    );
    for (index, expected) in ["a", "-", "", "b", "", "+", "c"].into_iter().enumerate() {
        let key = runtime.intern_property_key(&index.to_string()).unwrap();
        let value = context.get_property(&result, &key).unwrap();
        if matches!(index, 2 | 4) {
            assert_eq!(value, Value::Undefined);
        } else {
            assert_eq!(
                value,
                Value::String(JsString::try_from_utf8(expected).unwrap())
            );
        }
    }
    for value in arguments.readable {
        runtime.release_jsvalue(value).unwrap();
    }
    invocation.release(&runtime).unwrap();
}

fn count(runtime: &Runtime, object: &ObjectRef) -> u32 {
    runtime
        .0
        .state
        .borrow()
        .heap
        .object_strong_count(object.object_id())
        .unwrap()
}

#[test]
fn regexp_split_resident_abandonment_releases_duplicate_request_and_state_owners() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let token = context.new_object().unwrap();
    let constructor = match runtime
        .constructor_from_value(context.realm, context.eval("(function(){})").unwrap())
        .unwrap()
    {
        NativeConversion::Value(constructor) => constructor,
        NativeConversion::Throw(_) => panic!("ordinary function was not constructible"),
    };
    let before = count(&runtime, &token);
    for request in 0..6 {
        let state = SplitState {
            input_value: JsValue::Object(token.clone().into_handle()),
            input: JsString::from_static("a"),
            splitter: token.clone(),
            result: token.clone(),
            unicode: false,
            limit: 10,
            length: 0,
            p: 0,
            q: 0,
        };
        let mut resume = RegExpSplitResume::new(&runtime, context.realm, Phase::Exec(state));
        // Exercise the independent raw owners too, including aliases. Dropping
        // an unconsumed request must release occurrences, not unique identities.
        resume.0.input_value = JsValue::Object(token.clone().into_handle());
        resume.0.limit_value = JsValue::Object(token.clone().into_handle());
        let key = runtime.intern_property_key("test").unwrap();
        let step = match request {
            0 => RegExpSplitStep::make_primitive(
                JsValue::Object(token.clone().into_handle()),
                ToPrimitiveHint::Number,
                resume,
            ),
            1 => RegExpSplitStep::make_read(token.clone(), key, resume),
            2 => RegExpSplitStep::make_species(token.clone(), resume),
            3 => {
                resume.0.step_pending.arguments = Some(vec![
                    JsValue::Object(token.clone().into_handle()),
                    JsValue::Object(token.clone().into_handle()),
                ]);
                resume.0.step_pending.constructor = Some(constructor.clone());
                RegExpSplitStep::Construct { resume }
            }
            4 => RegExpSplitStep::make_set(
                token.clone(),
                key,
                JsValue::Object(token.clone().into_handle()),
                resume,
            ),
            5 => RegExpSplitStep::make_exec(
                JsValue::Object(token.clone().into_handle()),
                JsValue::Object(token.clone().into_handle()),
                resume,
            ),
            _ => unreachable!(),
        };
        assert!(count(&runtime, &token) > before);
        if let RegExpSplitStep::Exec { mut resume } = step {
            // A caller owns a taken field even if the remaining request is
            // abandoned. It must survive the resident state's destruction.
            let taken = resume.take_exec_regexp();
            drop(resume);
            assert_eq!(count(&runtime, &token), before + 1);
            runtime.release_jsvalue(taken).unwrap();
        } else {
            drop(step);
        }
        assert_eq!(
            count(&runtime, &token),
            before,
            "abandoned request {request}"
        );
    }
}

#[test]
fn regexp_split_resident_preserves_selected_exec_proxy_and_capture_boundaries() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    assert_eq!(
        context
            .eval(
                r#"(()=>{
        let trace='', token={}, target={lastIndex:0};
        const matched={
            get length(){trace+='G;';return {valueOf(){trace+='g;';return 3;}};},
            get 1(){trace+='A;';return token;},
            get 2(){trace+='B;';return undefined;}
        };
        const splitter=new Proxy(target,{
            set(o,k,v){trace+='W'+v+';';o[k]=v;return true;},
            get(o,k){
                if(k==='exec'){
                    const q=o.lastIndex;trace+='X'+q+';';
                    return function(input){
                        trace+='E'+q+';';if(q===0)return null;
                        o.lastIndex={valueOf(){trace+='D;';return 2;}};
                        return matched;
                    };
                }
                if(k==='lastIndex')trace+='R;';
                return o[k];
            }
        });
        const re={
            get constructor(){trace+='C;';return {
                get [Symbol.species](){trace+='S;';return function(re,flags){
                    trace+='N:'+flags+';';return splitter;
                };}
            };},
            get flags(){trace+='F;';return {toString(){trace+='f;';return '';}};}
        };
        const input={toString(){trace+='I;';return 'abc';}};
        const limit={valueOf(){trace+='L;';return 3;}};
        const result=RegExp.prototype[Symbol.split].call(re,input,limit);
        return result.length===3 && result[0]==='a' && result[1]===token
            && result[2]===undefined
            && trace==='I;C;S;F;f;N:y;L;W0;X0;E0;W1;X1;E1;R;D;G;g;A;B;';
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}

#[test]
fn regexp_split_resident_keeps_empty_unicode_limit_and_abrupt_unwind_behavior() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    assert_eq!(
        context
            .eval(
                r#"(()=>{
        const boundary='😀x'.split(/(?:)/u);
        const units='😀x'.split(/(?:)/);
        let calls=0;
        const splitter={lastIndex:0,exec(){calls++;throw 73;}};
        const re={flags:'',constructor:{[Symbol.species]:function(){return splitter;}}};
        const zero=RegExp.prototype[Symbol.split].call(re,'abc',0);
        let caught=false;
        try{RegExp.prototype[Symbol.split].call(re,'abc');}catch(e){caught=e===73;}
        let captureCaught=false;
        splitter.exec=function(){this.lastIndex=1;return {
            length:2,get 1(){throw 74;}
        };};
        try{RegExp.prototype[Symbol.split].call(re,'ab');}catch(e){captureCaught=e===74;}
        return boundary.length===2 && boundary[0]==='😀' && boundary[1]==='x'
            && units.length===3 && ''.split(/x/).length===1
            && ''.split(/(?:)/).length===0 && zero.length===0
            && calls===1 && caught && captureCaught;
    })()"#
            )
            .unwrap(),
        Value::Bool(true)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(context.eval("6*7").unwrap(), Value::Int(42));
}

#[test]
fn regexp_split_resident_results_and_late_set_errors_keep_the_defining_realm() {
    let runtime = Runtime::new();
    let mut defining = runtime.new_context();
    let Value::Object(function) = defining.eval("RegExp.prototype[Symbol.split]").unwrap() else {
        panic!("RegExp split was not a function");
    };
    let split = runtime.as_callable(&function).unwrap().unwrap();
    let array_prototype = defining.array_prototype().unwrap();
    let Value::Object(type_error_prototype) = defining.eval("TypeError.prototype").unwrap() else {
        panic!("TypeError prototype was not an object");
    };
    let mut caller = runtime.new_context();
    let regexp = caller.eval("/-/").unwrap();
    let Value::Object(result) = caller
        .call(
            &split,
            regexp,
            &[Value::String(JsString::from_static("a-b"))],
        )
        .unwrap()
    else {
        panic!("cross-realm split did not return an Array");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(array_prototype)
    );
    let regexp = caller
        .eval(
            r#"({flags:'', constructor:{[Symbol.species]:function(){
        return new Proxy({lastIndex:0},{set(){return false;}});
    }}})"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(&split, regexp, &[Value::String(JsString::from_static("a"))],),
        Err(RuntimeError::Exception)
    );
    let Some(Value::Object(error)) = caller.take_exception().unwrap() else {
        panic!("failed splitter write did not throw an Error");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(type_error_prototype)
    );
    assert!(runtime.0.state.borrow().active_frames.is_empty());
    assert_eq!(caller.eval("40+2").unwrap(), Value::Int(42));
}

#[cfg(feature = "profiling")]
#[test]
fn regexp_split_resident_production_loop_allocates_one_box_per_split() {
    use crate::engine::api::profiling::CostProfile;
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let profile = CostProfile::start();
    assert_eq!(
        context.eval(r#"'a-b-c'.split(/(-)/).join('|')"#).unwrap(),
        Value::String(JsString::from_static("a|-|b|-|c"))
    );
    let events = profile.snapshot().owned_execution_events;
    let count = |name| events.get(name).copied().unwrap_or(0);
    assert_eq!(count("regexp_split.resident_box"), 1);
    assert_eq!(count("regexp_split.next_box_reused"), 5);
    assert_eq!(count("regexp_split.exec_box_reused"), 5);
    assert_eq!(count("regexp_split.capture_box_reused"), 2);
    assert!(runtime.0.state.borrow().active_frames.is_empty());
}
