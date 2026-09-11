//! Published execution must retain dynamic binding transitions after static
//! access-mode checks move to publication. These tests use the real compiler,
//! publisher and runtime host, not synthetic instruction fixtures.
use crate::engine::api::runtime::Runtime;
use crate::engine::value::Value;

#[test]
fn invalid_binding_modes_are_rejected_before_creating_a_runtime_frame() {
    use crate::engine::code::bytecode::Instruction;
    use crate::engine::code::function::metadata::FunctionMetadata;
    use crate::engine::code::function::{UnlinkedFunction, UnlinkedVariableDefinition};
    use crate::engine::value::JsString;

    let cases = [
        (
            Instruction::GetLocal(0),
            UnlinkedVariableDefinition::lexical(Some(JsString::from_static("n")), false),
            "unchecked local opcode referenced a lexical definition",
        ),
        (
            Instruction::GetLocalCheck(0),
            UnlinkedVariableDefinition::ordinary(None),
            "checked lexical-local opcode referenced an ordinary definition",
        ),
    ];
    for (instruction, definition, expected) in cases {
        let runtime = Runtime::new();
        let context = runtime.new_context();
        let before = runtime.heap_counts().function_bytecode_nodes;
        let function = UnlinkedFunction::fixture(
            vec![instruction, Instruction::Return],
            vec![],
            FunctionMetadata {
                local_count: 1,
                max_stack: 1,
                ..FunctionMetadata::default()
            },
        )
        .with_fixture_definitions(vec![], vec![definition]);
        let error = runtime
            .publish_unlinked_function(context.realm, function)
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(runtime.heap_counts().function_bytecode_nodes, before);
    }
}

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
