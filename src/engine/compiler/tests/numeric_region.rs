use super::*;
use crate::engine::code::exec::without_numeric_regions;
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::vm::test_numeric_region_hits;

fn run(source: &str, generic: bool) -> (Value, usize) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let (result, hits) = test_numeric_region_hits(|| {
        if generic {
            without_numeric_regions(|| context.eval(source))
        } else {
            context.eval(source)
        }
    });
    (
        result.unwrap_or_else(|error| panic!("{source}: {error:?}")),
        hits,
    )
}

fn opcodes(source: &str) -> Vec<Opcode> {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let root = context.compile(source).unwrap();
    let child = runtime.test_child_function_bytecode(&root, 0).unwrap();
    runtime.test_function_exec_opcodes(&child).unwrap()
}

fn assert_same(source: &str, expected_hits: usize) -> Value {
    let (fast, hits) = run(source, false);
    let (generic, generic_hits) = run(source, true);
    assert_eq!(format!("{fast:?}"), format!("{generic:?}"), "{source}");
    assert_eq!(hits, expected_hits, "{source}");
    assert_eq!(generic_hits, 0, "{source}");
    fast
}

#[test]
fn selected_region_executes_and_suppressed_selection_uses_generic_words() {
    let source = "(function(){ function f(array,i,scale){var sum=7;sum += array[i]*scale;return sum;} return f([2],0,3); })()";
    let (fast, hits) = run(source, false);
    let (generic, generic_hits) = run(source, true);
    assert_eq!(fast, Value::Int(13));
    assert_eq!(fast, generic);
    assert_eq!(hits, 1);
    assert_eq!(generic_hits, 0);
}

#[test]
fn numeric_region_falls_back_without_changing_observable_access() {
    let cases = [
        // Own data, hole, accessor, proxy, and coercion all take their normal paths.
        "var a=[2]; Object.defineProperty(a,'0',{value:4,writable:false}); return f(a,0,3);",
        "return f([,],0,3);",
        "var log=''; var a=[]; Object.defineProperty(a,'0',{get(){log+='g'; return 4;}}); return [f(a,0,3),log].join(':');",
        "var log=''; var a=new Proxy([4],{get(t,k,r){log+=String(k);return Reflect.get(t,k,r)}}); return [f(a,0,3),log].join(':');",
        "var log=''; var a=[{valueOf(){log+='e';return 4}}]; var s={valueOf(){log+='s';return 3}}; return [f(a,0,s),log].join(':');",
        "var a=Object.freeze([4]); return f(a,0,3);",
        "return f([4],0,-0);",
        "return f([Infinity],0,0);",
        "return f([NaN],0,3);",
    ];
    for case in cases {
        let source = format!(
            "(function(){{function f(a,i,s){{var sum=7;sum += a[i]*s;return sum;}} {case}}})()"
        );
        let (fast, _) = run(&source, false);
        let (generic, _) = run(&source, true);
        assert_eq!(format!("{fast:?}"), format!("{generic:?}"), "{case}");
    }
}

#[test]
fn numeric_region_preserves_number_edges_aliases_and_storage_modes() {
    let cases = [
        ("var a=[4]; return f(a,0,3);", 1),
        (
            "var a=[4]; Object.defineProperty(a,'0',{value:4,writable:false}); return f(a,0,3);",
            1,
        ),
        ("var a=Object.freeze([4]); return f(a,0,3);", 1),
        ("return f([1],0,2147483647);", 1),
        ("return f([-0],0,1);", 1),
        ("return f([NaN],0,2);", 1),
        ("return f([Infinity],0,2);", 1),
        ("return f([4],0,0);", 1),
    ];
    for (case, hits) in cases {
        let source = format!(
            "(function(){{function f(a,i,s){{var sum=7;sum += a[i]*s;return sum;}} {case}}})()"
        );
        let _ = assert_same(&source, hits);
    }
    let alias =
        "(function(){function f(a){var sum=2;sum += a[sum]*sum;return sum;}return f([1,3,5]);})()";
    assert_eq!(assert_same(alias, 1), Value::Int(12));
    let direct_locals = "(function(){function f(){var a=[3],i=0,s=4,sum=5;sum += a[i]*s;return sum;}return f();})()";
    assert_eq!(assert_same(direct_locals, 1), Value::Int(17));
    let constants =
        "(function(){function f(a){var sum=7;sum += a[0]*1.5;return sum;}return f([4]);})()";
    assert_eq!(assert_same(constants, 1), Value::Int(13));
    let signed_zero = "(function(){function f(a,i,s){var sum=-0;sum += a[i]*s;return Object.is(sum,-0);}return f([-0],0,1);})()";
    assert_eq!(assert_same(signed_zero, 1), Value::Bool(true));
}

#[test]
fn numeric_region_misses_preserve_errors_and_evaluation_order() {
    let cases = [
        "var log=''; var a=[]; Object.defineProperty(a,'0',{get(){log+='read';throw Error('boom');}}); try {f(a,0,2);} catch(e) {return log+':'+e.message;}",
        "var log=''; var a=[{valueOf(){log+='element'; return 2;}}]; var s={valueOf(){log+='scale';return 3;}}; var value=f(a,0,s); return log+':'+value;",
        "var log=''; var a=new Proxy([2],{get(t,k,r){log+='proxy';return Reflect.get(t,k,r);}}); return f(a,0,3)+':'+log;",
        "return f([,],0,3);",
        "return f([2],-1,3);",
        "return f([2],4294967295,3);",
    ];
    for case in cases {
        let source = format!(
            "(function(){{function f(a,i,s){{var sum=7;sum += a[i]*s;return sum;}} {case}}})()"
        );
        let _ = assert_same(&source, 0);
    }
}

#[test]
fn fallback_preserves_fault_source_position() {
    let source = r#"(function(){
        function f(a,i,s){
            var sum=7;
            sum += a[i]*s;
            return sum;
        }
        var a=[];
        Object.defineProperty(a,'0',{get(){throw Error('m1-source');}});
        try {f(a,0,2);} catch(error) {return error.message+':'+String(error.stack);}
    })()"#;
    let stack = assert_same(source, 0);
    assert!(format!("{stack:?}").contains("m1-source"));
}

#[test]
fn uncertain_binding_shapes_are_not_selected() {
    let lexical = opcodes("(function(a,i,s){let sum=0;sum += a[i]*s;return sum;})");
    assert!(!lexical.contains(&Opcode::NumericArrayAccumulate));
    let captured = opcodes(
        "(function(a,i,s){var sum=0;function read(){return sum;}sum += a[i]*s;return read();})",
    );
    assert!(!captured.contains(&Opcode::NumericArrayAccumulate));
    let eval = opcodes("(function(a,i,s){var sum=0;eval('sum');sum += a[i]*s;return sum;})");
    assert!(!eval.contains(&Opcode::NumericArrayAccumulate));
    let mapped = opcodes("(function(a,i,s){var sum=0;arguments[0]=a;sum += a[i]*s;return sum;})");
    assert!(!mapped.contains(&Opcode::NumericArrayAccumulate));
}

#[test]
fn resumed_async_frame_enters_region_after_await() {
    use crate::engine::jobs::PendingJobOutcome;
    let source = "(function(){globalThis.m1Result=0;async function f(a,i,s){var sum=7;await 0;sum += a[i]*s;return sum;}f([2],0,3).then(value=>{m1Result=value;});})()";
    for generic in [false, true] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context();
        let (_, hits) = test_numeric_region_hits(|| {
            if generic {
                let _ = without_numeric_regions(|| context.eval(source)).unwrap();
            } else {
                let _ = context.eval(source).unwrap();
            }
            for _ in 0..16 {
                if runtime.execute_pending_job().unwrap() == PendingJobOutcome::NoJob {
                    break;
                }
            }
        });
        assert_eq!(context.eval("m1Result").unwrap(), Value::Int(13));
        assert_eq!(hits, usize::from(!generic));
    }
}
