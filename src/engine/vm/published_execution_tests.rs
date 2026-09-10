//! Published execution must retain dynamic binding transitions after static
//! access-mode checks move to publication. These tests use the real compiler,
//! publisher and runtime host, not synthetic instruction fixtures.
use crate::engine::api::runtime::Runtime;
use crate::engine::value::Value;

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
