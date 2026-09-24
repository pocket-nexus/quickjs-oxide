//! Published execution must retain dynamic binding transitions after static
//! access-mode checks move to publication. These tests use the real compiler,
//! publisher and runtime host, not synthetic instruction fixtures.
use crate::engine::api::runtime::Runtime;
use crate::engine::value::{JsValue, Value};

#[test]
fn published_bindings_keep_capture_eval_and_argument_aliases_live() {
    for (source, expected) in [
        (
            "(function(){var n=2; n+=3; var f=()=>n; n+=7; return f();})()",
            12,
        ),
        (
            "(function(){var n=2; eval('var f=()=>n'); n=7; return f();})()",
            7,
        ),
        (
            "(function(a){arguments[0]=7; return a+arguments.length;})(1,2,3)",
            10,
        ),
        (
            "(function(a){'use strict'; arguments[0]=7; return a+arguments.length;})(1,2,3)",
            4,
        ),
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert!(
            matches!(context.eval(source).unwrap(), Value::Int(n) if n == expected),
            "{source}"
        );
    }
}

#[test]
fn published_lexical_reads_preserve_tdz_then_observe_initialization() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let value = context
        .eval(
            "(function(){
        var f=()=>n; var caught=0;
        try { f(); } catch(e) { if (!(e instanceof ReferenceError)) throw e; caught=1; }
        let n=41; return f()+caught;
    })()",
        )
        .unwrap();
    assert!(matches!(value, Value::Int(42)));
}

#[test]
fn published_resume_keeps_captured_cells_live_through_finally() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let value = context
        .eval(
            "(function(){
        function* g(){let n=2; let f=()=>n; yield f; try {yield ++n;} finally {n=9;}}
        var it=g(); var f=it.next().value;
        if(f()!==2 || it.next().value!==3) throw Error('resume');
        it.return(0); return f()+1;
    })()",
        )
        .unwrap();
    assert!(matches!(value, Value::Int(10)));
}

#[test]
fn published_lexical_writes_preserve_tdz_const_and_iteration_lifetimes() {
    for source in [
        "(function(){let x=1; x=4; return x===4;})()",
        "(function(){let x=1; const set=v=>x=v; set(4); return x===4;})()",
        "(function(){const set=v=>x=v; let caught=false; try{set(4)}catch(e){caught=e instanceof ReferenceError} let x=1; set(5); return caught&&x===5;})()",
        "(function(){let caught=false; try{x=4}catch(e){caught=e instanceof ReferenceError} let x=1; return caught&&x===1;})()",
        "(function(){const x=1; try{x=4}catch(e){return e instanceof TypeError&&x===1} return false;})()",
        "(function(){const x=1; const set=v=>x=v; try{set(4)}catch(e){return e instanceof TypeError&&x===1} return false;})()",
        "(function(){let fs=[]; for(let i=0;i<3;i++){let x=i; fs.push(()=>++x)} return fs[0]()===1&&fs[1]()===2&&fs[2]()===3&&fs[0]()===2;})()",
        "(function(){let x=1; eval('x=4'); return x===4;})()",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert!(
            matches!(context.eval(source).unwrap(), Value::Bool(true)),
            "{source}"
        );
    }
}

#[test]
fn published_eval_reuses_topology_but_observes_live_scope_and_super() {
    for source in [
        "(function(a){let x=2; const f=()=>eval('x+=a'); f(); return x===5;})(3)",
        "(function(){let x=1; let o={x:4}; with(o){eval('x+=2')} return x===1&&o.x===6;})()",
        "(function(){let fs=[]; for(let i=0;i<3;i++){let x=i; fs.push(eval('()=>++x'))} return fs[0]()===1&&fs[1]()===2&&fs[2]()===3;})()",
        "(function(){class A{m(){return 2}} class B extends A{m(){return eval('super.m()')+1}} return new B().m()===3;})()",
        "(function(){class A{constructor(){this.n=3}} class B extends A{constructor(){eval('super()')}} return new B().n===3;})()",
        "(function(){let x=1; eval(\"eval('x=4')\"); return x===4;})()",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert!(
            matches!(context.eval(source).unwrap(), Value::Bool(true)),
            "{source}"
        );
    }
}

#[test]
fn repeated_closure_creation_reuses_cells_without_erasing_their_metadata() {
    for source in [
        "(function(){let x=1; let a=()=>x; let b=()=>++x; return b()===2&&a()===2;})()",
        "(function(){let fs=[]; for(let i=0;i<3;i++){let x=i; fs.push(()=>x,()=>++x)} return fs[1]()===1&&fs[0]()===1&&fs[3]()===2&&fs[2]()===2;})()",
        "(function named(){let a=()=>named; let b=eval('()=>named'); named=1; return a()===b()&&typeof a()==='function';})()",
        "(function(){class A{#x=3; read(){return [()=>this.#x,()=>this.#x]}} let fs=new A().read(); return fs[0]()===3&&fs[1]()===3;})()",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert!(
            matches!(context.eval(source).unwrap(), Value::Bool(true)),
            "{source}"
        );
    }
}

#[test]
fn published_static_branches_preserve_resume_finally_and_loop_targets() {
    for source in [
        "(function(){let n=0; do {++n} while(n<3); while(n>1){--n} return n===1;})()",
        "(function(){let n=0; for(let i=0;i<5;i++){try{if(i%2)continue; n+=i;}finally{++n;}}return n===11;})()",
        "(function(){function* g(){let n=0;try{while(n<3){yield n++;}}finally{n=9;}return n;}let it=g();return it.next().value===0&&it.next().value===1&&it.return(7).value===7;})()",
    ] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        assert!(
            matches!(context.eval(source).unwrap(), Value::Bool(true)),
            "{source}"
        );
    }
}

#[test]
fn stack_reads_cover_full_depth_range_and_preserve_root_ownership() {
    use super::stack::{FrameStorage, SlotStore};
    use crate::engine::code::runtime::PublishedFunctionSnapshot;
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    for length in [0usize, 1, 255, 256, 257] {
        let mut code = PublishedFunctionSnapshot::empty_for_test(context.realm);
        code.metadata.max_stack = length as u16;
        let mut slots = SlotStore::new(length + 1);
        let mut window = slots
            .push_frame(
                &runtime,
                &code.frame_layout(),
                FrameStorage {
                    original_arguments: vec![],
                    parameters: vec![],
                    locals: vec![],
                    operands: vec![],
                },
            )
            .unwrap();
        for index in 0..length {
            slots.push(&mut window, JsValue::Int(index as i32)).unwrap();
        }
        for depth in 0..=255 {
            let value = slots.peek(&window, depth);
            if depth < length {
                assert_eq!(value.unwrap(), &JsValue::Int((length - depth - 1) as i32));
            } else {
                assert!(value.is_err());
            }
        }
        slots.clear_frame(&runtime, window).unwrap();
    }
    let object = context.new_object().unwrap();
    let id = object.object_id();
    let mut code = PublishedFunctionSnapshot::empty_for_test(context.realm);
    code.metadata.max_stack = 2;
    let mut slots = SlotStore::new(2);
    let mut window = slots
        .push_frame(
            &runtime,
            &code.frame_layout(),
            FrameStorage {
                original_arguments: vec![],
                parameters: vec![],
                locals: vec![],
                operands: vec![],
            },
        )
        .unwrap();
    slots
        .push(
            &mut window,
            runtime.into_jsvalue(Value::Object(object)).unwrap(),
        )
        .unwrap();
    slots.push(&mut window, JsValue::Int(9)).unwrap();
    let saved = runtime
        .dup_jsvalue(slots.peek(&window, 1).unwrap())
        .unwrap();
    assert_eq!(slots.pop(&mut window).unwrap(), JsValue::Int(9));
    slots.clear_frame(&runtime, window).unwrap();
    assert!(runtime.0.state.borrow().heap.object(id).is_ok());
    runtime.release_jsvalue(saved).unwrap();
    assert!(runtime.0.state.borrow().heap.object(id).is_err());
}
