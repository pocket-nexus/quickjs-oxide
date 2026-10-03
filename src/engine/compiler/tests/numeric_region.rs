use super::*;
use crate::engine::code::exec::without_numeric_regions;
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::vm::test_numeric_region_counts;
use crate::engine::vm::test_numeric_region_hits;

fn run(source: &str, generic: bool) -> (Value, usize) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
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
    let mut context = runtime.new_context().expect("create context");
    let root = context.compile(source).unwrap();
    let child = runtime.test_child_function_bytecode(&root, 0).unwrap();
    runtime.test_function_exec_opcodes(&child).unwrap()
}

#[test]
fn m2_regions_select_and_match_generic_execution() {
    let selections = [
        (
            "(function(a,i,s){var out=0;out=a[i]*s;return out;})",
            Opcode::NumericArrayStoreProduct,
        ),
        (
            "(function(a,i,d){a[i]+=d;return a[i];})",
            Opcode::NumericArrayUpdateElement,
        ),
        (
            "(function(a,i,l){if(a[i]<l)return 1;return 0;})",
            Opcode::NumericArrayCompareBranch,
        ),
        (
            "(function(a,i,s){let sum=0;sum+=a[i]*s;return sum;})",
            Opcode::NumericArrayAccumulate,
        ),
    ];
    for (source, opcode) in selections {
        assert!(opcodes(source).contains(&opcode), "{source}");
    }
    for (source, expected) in [
        (
            "(function(){function f(a,i,s){var out=0;out=a[i]*s;return out;}return f([4],0,3);})()",
            Value::Int(12),
        ),
        (
            "(function(){function f(a,i,d){a[i]+=d;return a[i];}return f([4],0,3);})()",
            Value::Int(7),
        ),
        (
            "(function(){function f(a,i,l){if(a[i]<l)return 1;return 0;}return f([4],0,5);})()",
            Value::Int(1),
        ),
        (
            "(function(){function f(a,i,s){let sum=0;sum+=a[i]*s;return sum;}return f([4],0,3);})()",
            Value::Int(12),
        ),
    ] {
        assert_eq!(assert_same(source, 1), expected);
    }
}

#[test]
fn product_operand_order_matches_publication_and_fallback() {
    let array_first_store = "(function(a,i,s){var out=0;out=a[i]*s;return out;})";
    let scale_first_store = "(function(a,i,s){var out=0;out=s*a[i];return out;})";
    assert!(opcodes(array_first_store).contains(&Opcode::NumericArrayStoreProduct));
    assert!(!opcodes(scale_first_store).contains(&Opcode::NumericArrayStoreProduct));

    let scale_first_update = "(function(x,s,dt,i){x[i]+=dt*s[i];return x[i];})";
    let array_first_update = "(function(x,s,dt,i){x[i]+=s[i]*dt;return x[i];})";
    assert!(opcodes(scale_first_update).contains(&Opcode::NumericArrayUpdateElement));
    assert!(!opcodes(array_first_update).contains(&Opcode::NumericArrayUpdateElement));

    for (expression, hits) in [("a[i]*s", 1), ("s*a[i]", 0)] {
        let source = format!(
            "(function(){{function f(a,i,s){{var out=0;out={expression};return out;}}return f([4],0,3);}})()"
        );
        assert_eq!(assert_same(&source, hits), Value::Int(12));
        let getter = format!(
            "(function(){{function f(a,i,s){{var out=0;out={expression};return out;}}var log='';var a=[];Object.defineProperty(a,'0',{{get(){{log+='g';return 4;}}}});return f(a,0,3)+':'+log;}})()"
        );
        let _ = assert_same(&getter, 0);
        let coercion = format!(
            "(function(){{function f(a,i,s){{var out=0;out={expression};return out;}}var log='';var a=[{{valueOf(){{log+='a';return 4;}}}}],s={{valueOf(){{log+='s';return 3;}}}};return f(a,0,s)+':'+log;}})()"
        );
        let _ = assert_same(&coercion, 0);
    }
    for (expression, hits) in [("dt*s[i]", 1), ("s[i]*dt", 0)] {
        let source = format!(
            "(function(){{function f(x,s,dt,i){{x[i]+={expression};return x[i];}}return f([1],[3],2,0);}})()"
        );
        assert_eq!(assert_same(&source, hits), Value::Int(7));
        let getter = format!(
            "(function(){{function f(x,s,dt,i){{x[i]+={expression};return x[i];}}var log='';var s=[];Object.defineProperty(s,'0',{{get(){{log+='g';return 3;}}}});return f([1],s,2,0)+':'+log;}})()"
        );
        let _ = assert_same(&getter, 0);
        let coercion = format!(
            "(function(){{function f(x,s,dt,i){{x[i]+={expression};return x[i];}}var log='';var s=[{{valueOf(){{log+='s';return 3;}}}}],dt={{valueOf(){{log+='d';return 2;}}}};return f([1],s,dt,0)+':'+log;}})()"
        );
        let _ = assert_same(&coercion, 0);
    }
}

#[test]
fn array_product_update_selects_and_preserves_aliases_and_misses() {
    assert!(
        opcodes("(function(x,s,dt){for(var i=0;i<3;i++)x[i]+=dt*s[i];return x[0];})")
            .contains(&Opcode::NumericArrayUpdateElement)
    );
    for (body, hits) in [
        ("var x=[1,2];var s=[3,4];return f(x,s,2).join(',');", 2),
        ("var x=[1,2];return f(x,x,2).join(',');", 2),
        (
            "var x=[1,2];return f(x,Object.freeze([3,4]),2).join(',');",
            2,
        ),
        ("var x=[1,2];return f(x,[,4],2).join(',');", 1),
        (
            "var x=[1,2];var s=new Proxy([3,4],{get(t,k,r){return Reflect.get(t,k,r)}});return f(x,s,2).join(',');",
            0,
        ),
    ] {
        let source = format!(
            "(function(){{function f(x,s,dt){{for(var i=0;i<2;i++)x[i]+=dt*s[i];return x;}}{body}}})()"
        );
        let _ = assert_same(&source, hits);
    }
    let side_effect = r#"(function(){
        function f(x,s,dt){x[0]+=dt*s[0];return x[0];}
        var x=[1], s=[{valueOf(){x[0]=100;return 3;}}];
        return f(x,s,2);
    })()"#;
    assert_eq!(assert_same(side_effect, 0), Value::Int(7));
    let strict_write = r#"(function(){
        function f(x,s,dt){'use strict';x[0]+=dt*s[0];}
        try { f(Object.freeze([1]),[3],2); return false; }
        catch (e) { return e instanceof TypeError; }
    })()"#;
    assert_eq!(assert_same(strict_write, 0), Value::Bool(true));
    let thrown_getter = r#"(function(){
        var sentinel={};
        function f(x,s,dt){x[0]+=dt*s[0];}
        var s=[];Object.defineProperty(s,'0',{get(){throw sentinel;}});
        try { f([1],s,2); return false; }
        catch (e) { return e===sentinel; }
    })()"#;
    assert_eq!(assert_same(thrown_getter, 0), Value::Bool(true));
}

#[test]
fn array_copy_region_preserves_source_read_and_target_write_order() {
    assert!(
        opcodes("(function(x,x0,i){x[i]=x0[i];return x[i];})")
            .contains(&Opcode::NumericArrayCopyElement)
    );
    for (body, hits) in [
        ("return f([1],[4],0);", 1),
        ("var x=[4];return f(x,x,0);", 1),
        (
            "var x=[1],s=[];Object.defineProperty(s,'0',{get(){return 4;}});return f(x,s,0);",
            0,
        ),
        (
            "var x=[1],s=[{valueOf(){return 4;}}];return String(f(x,s,0));",
            0,
        ),
        (
            "var x=new Proxy([1],{set(t,k,v,r){return Reflect.set(t,k,v,r)}});return f(x,[4],0);",
            0,
        ),
    ] {
        let source =
            format!("(function(){{function f(x,x0,i){{x[i]=x0[i];return x[i];}}{body}}})()");
        let _ = assert_same(&source, hits);
    }
    let strict_frozen = "(function(){function f(x,x0){'use strict';x[0]=x0[0];}try{f(Object.freeze([1]),[4]);return false;}catch(e){return e instanceof TypeError;}})()";
    assert_eq!(assert_same(strict_frozen, 0), Value::Bool(true));
    let getter_order = "(function(){var log='';function f(x,s){x[0]=s[0];return x[0];}var x=[1],s=[];Object.defineProperty(s,'0',{get(){log+='g';return 4;}});return f(x,s)+':'+log;})()";
    let _ = assert_same(getter_order, 0);
}

#[test]
fn array_add_preinc_consumes_stack_number_and_preserves_miss_effects() {
    let function = "(function(a){var i=0,sum=1;sum+=a[++i];return sum+':'+i;})";
    assert!(opcodes(function).contains(&Opcode::NumericArrayAddPreInc));
    let hit = "(function(){function f(a){var i=0,sum=1;sum+=a[++i];return sum+':'+i;}return f([0,4]);})()";
    let _ = assert_same(hit, 1);
    let getter = "(function(){var log='';function f(a){var i=0,sum=1;sum+=a[++i];return sum+':'+i+':'+log;}var a=[];Object.defineProperty(a,'1',{get(){log+='g';return 4;}});return f(a);})()";
    let _ = assert_same(getter, 0);
    let coercion = "(function(){var log='';function f(a){var i=0,sum=1;sum+=a[++i];return sum+':'+i+':'+log;}return f([0,{valueOf(){log+='v';return 4;}}]);})()";
    let _ = assert_same(coercion, 0);
    let index_miss = "(function(){function f(a){var i='0',sum=1;sum+=a[++i];return sum+':'+i;}return f([0,4]);})()";
    let _ = assert_same(index_miss, 0);
}

#[test]
fn array_store_and_local_uses_saved_stack_target_and_falls_back_cleanly() {
    let function = "(function(a,i,v){var out=0;out=a[i]=v;return out;})";
    assert!(opcodes(function).contains(&Opcode::NumericArrayStoreAndLocal));
    for (body, hits) in [
        ("return f([1],0,4);", 1),
        ("var a=[1];return f(a,0,4)+':'+a[0];", 1),
        (
            "var a=Object.freeze([1]);try{f(a,0,4);return false;}catch(e){return e instanceof TypeError;}",
            0,
        ),
        (
            "var log='';var a=[1];Object.defineProperty(a,'0',{get(){log+='g';return 1;},set(v){log+='s';}});return f(a,0,4)+':'+log;",
            0,
        ),
    ] {
        let source = format!(
            "(function(){{function f(a,i,v){{'use strict';var out=0;out=a[i]=v;return out;}}{body}}})()"
        );
        let _ = assert_same(&source, hits);
    }
    let saved_index = "(function(){function f(a){var i=0,out=0;out=a[i]=++i;return out+':'+i+':'+a.join(',');}return f([1,2]);})()";
    // State-held retirement consumes the saved receiver after committing both
    // stores; this does not need an observable frame.
    let _ = assert_same(saved_index, 1);
}

#[test]
fn product_assignment_replaces_only_initialized_owner_free_scalars() {
    for old in ["undefined", "null", "false", "true", "0", "-0"] {
        let source = format!(
            "(function(){{function f(a,i,s){{var out={old};out=a[i]*s;return out;}}return f([4],0,3);}})()"
        );
        assert_eq!(assert_same(&source, 1), Value::Int(12));
    }
    let lexical = "(function(){function f(a){let out;out=a[0]*3;return out;}return f([4]);})()";
    assert_eq!(assert_same(lexical, 1), Value::Int(12));
    for old in ["{}", "[]", "'owned'", "1n"] {
        let source = format!(
            "(function(){{function f(a,i,s){{var out={old};out=a[i]*s;return out;}}return f([4],0,3);}})()"
        );
        assert_eq!(assert_same(&source, 0), Value::Int(12));
    }
}

#[test]
fn dynamic_misses_attempt_the_selected_region() {
    let source =
        "(function(){function f(a,i,s){var sum=7;sum+=a[i]*s;return sum;}return f([,],0,3);})()";
    assert!(
        opcodes("(function(a,i,s){var sum=7;sum+=a[i]*s;return sum;})")
            .contains(&Opcode::NumericArrayAccumulate)
    );
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let (result, counts) = test_numeric_region_counts(|| context.eval(source));
    assert!(result.is_ok());
    assert_eq!(counts, (1, 0, 1));
}

#[test]
fn m2_array_write_modes_and_fallback_match_generic() {
    for (case, hits) in [
        ("return f([4],0,3);", 1),
        (
            "var a=[4];Object.defineProperty(a,'0',{value:4,writable:true,enumerable:false});return f(a,0,3);",
            1,
        ),
        ("return f(Object.freeze([4]),0,3);", 0),
        ("return f([,],0,3);", 0),
        (
            "var log='';var a=[4];Object.defineProperty(a,'0',{get(){log+='g';return 4},set(v){log+='s'}});return String(f(a,0,3))+log;",
            0,
        ),
        (
            "var log='';var a=new Proxy([4],{get(t,k,r){log+='g';return Reflect.get(t,k,r)},set(t,k,v,r){log+='s';return Reflect.set(t,k,v,r)}});return String(f(a,0,3))+log;",
            0,
        ),
    ] {
        let source = format!("(function(){{function f(a,i,d){{a[i]+=d;return a[i];}}{case}}})()");
        let _ = assert_same(&source, hits);
    }
}

#[test]
fn m2_branch_edges_and_fallback_match_generic() {
    for case in [
        "return f([4],0,5);",
        "return f([NaN],0,5);",
        "return f([4],0,NaN);",
        "return f([,],0,5);",
        "var a=new Proxy([4],{get(t,k,r){return Reflect.get(t,k,r)}});return f(a,0,5);",
    ] {
        let source =
            format!("(function(){{function f(a,i,l){{if(a[i]<l)return 1;return 0;}}{case}}})()");
        let _ = assert_same(
            &source,
            if case.starts_with("return f([4]") || case.contains("[NaN]") {
                1
            } else {
                0
            },
        );
    }
}

#[test]
fn numeric_comparison_branch_covers_all_supported_operators() {
    for operator in ["<", "<=", ">", ">=", "==", "!=", "===", "!=="] {
        let function =
            format!("(function(a,i,limit){{if(a[i]{operator}limit)return 1;return 0;}})");
        assert!(
            opcodes(&function).contains(&Opcode::NumericArrayCompareBranch),
            "{operator}"
        );
        for (array, limit, hits) in [
            ("[4]", "4", 1),
            ("[4]", "5", 1),
            ("[NaN]", "4", 1),
            ("[Infinity]", "-Infinity", 1),
            ("[,]", "4", 0),
        ] {
            let source = format!(
                "(function(){{function f(a,i,limit){{if(a[i]{operator}limit)return 1;return 0;}}return f({array},0,{limit});}})()"
            );
            let _ = assert_same(&source, hits);
        }
    }
}

#[test]
fn region_selection_preserves_neighbors_and_scales_with_many_sites() {
    let adjacent =
        opcodes("(function(a,i,s){var counter=0,sum=0;counter++;sum+=a[i]*s;return counter+sum;})");
    assert!(adjacent.contains(&Opcode::NumericArrayAccumulate));
    assert!(adjacent.contains(&Opcode::UpdateLocalDiscard));
    let mut source = String::from("(function(a,i,s){var sum=0;");
    for _ in 0..200 {
        source.push_str("sum+=a[i]*s;");
    }
    source.push_str("return sum;})");
    assert_eq!(
        opcodes(&source)
            .iter()
            .filter(|&&opcode| opcode == Opcode::NumericArrayAccumulate)
            .count(),
        200
    );
}

#[test]
fn array_update_site_handles_own_hole_accessor_and_own_transitions() {
    let source = r#"(function(){
        function f(a){a[0]+=1;return a[0];}
        var a=[2], log='';
        var first=f(a);
        delete a[0];
        var hole=f(a);
        Object.defineProperty(a,'0',{configurable:true,get(){log+='g';return 5},set(v){log+='s'}});
        var accessor=f(a);
        Object.defineProperty(a,'0',{configurable:true,writable:true,value:10});
        var last=f(a);
        return [first,String(hole),accessor,last,log].join(':');
    })()"#;
    let (generic, _) = run(source, true);
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let (optimized, counts) = test_numeric_region_counts(|| context.eval(source));
    assert_eq!(format!("{:?}", optimized.unwrap()), format!("{generic:?}"));
    assert_eq!(counts, (4, 2, 2));
}

#[test]
fn rejected_region_preserves_thrown_object_identity() {
    let source = r#"(function(){
        var sentinel={};
        function f(a){var sum=1;sum+=a[0]*2;return sum;}
        var a=[];
        Object.defineProperty(a,'0',{get(){throw sentinel}});
        try { f(a); return false; } catch (error) { return error===sentinel; }
    })()"#;
    assert_eq!(assert_same(source, 0), Value::Bool(true));
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
    assert!(lexical.contains(&Opcode::NumericArrayAccumulate));
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
fn initialized_lexical_sources_and_destinations_are_selected_only_after_initialization() {
    let initialized = "(function(a){const i=0,scale=3;let out=0;out=a[i]*scale;return out;})";
    assert!(opcodes(initialized).contains(&Opcode::NumericArrayStoreProduct));
    assert_eq!(
        assert_same(
            "(function(){function f(a){const i=0,scale=3;let out=0;out=a[i]*scale;return out;}return f([4]);})()",
            1,
        ),
        Value::Int(12)
    );
    let before_initialization = "(function(a){let out=0;out=a[i]*2;const i=0;return out;})";
    assert!(!opcodes(before_initialization).contains(&Opcode::NumericArrayStoreProduct));
    let source = "(function(){function f(a){let out=0;out=a[i]*2;const i=0;return out;}try{f([4]);return false;}catch(error){return error instanceof ReferenceError;}})()";
    assert_eq!(assert_same(source, 0), Value::Bool(true));
}

#[test]
fn initialized_lexical_destination_survives_ordinary_loop_edges() {
    let function =
        "(function(a,s){let sum=0;for(let i=0;i<a.length;i++){sum+=a[i]*s;}return sum;})";
    assert!(opcodes(function).contains(&Opcode::NumericArrayAccumulate));
    let source = "(function(){function f(a,s){let sum=0;for(let i=0;i<a.length;i++){sum+=a[i]*s;}return sum;}return f([2,3,4],2);})()";
    assert_eq!(assert_same(source, 3), Value::Int(18));
}

#[test]
fn array_update_reads_aliased_index_and_delta_before_writing() {
    let source = "(function(){function f(a,i){a[i]+=i;return a[i];}return f([4,5],1);})()";
    assert_eq!(assert_same(source, 1), Value::Int(6));
}

#[test]
fn resumed_async_frame_enters_region_after_await() {
    use crate::engine::jobs::PendingJobOutcome;
    let source = "(function(){globalThis.m1Result=0;async function f(a,i,s){var sum=7;await 0;sum += a[i]*s;return sum;}f([2],0,3).then(value=>{m1Result=value;});})()";
    for generic in [false, true] {
        let runtime = Runtime::new();
        let mut context = runtime.new_context().expect("create context");
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
