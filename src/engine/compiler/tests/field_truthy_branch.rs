use super::*;
use crate::engine::code::exec::without_field_truthy_branches;
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::vm::test_field_truthy_counts;

fn evaluate(source: &str, ordinary: bool) -> (Value, (usize, usize)) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let (result, counts) = test_field_truthy_counts(|| {
        if ordinary {
            without_field_truthy_branches(|| context.eval(source))
        } else {
            context.eval(source)
        }
    });
    (
        result.unwrap_or_else(|error| panic!("{source}: {error:?}")),
        counts,
    )
}

fn assert_same(source: &str) -> (Value, (usize, usize)) {
    let (fast, counts) = evaluate(source, false);
    let (ordinary, ordinary_counts) = evaluate(source, true);
    assert_eq!(format!("{fast:?}"), format!("{ordinary:?}"), "{source}");
    assert_eq!(ordinary_counts, (0, 0), "{source}");
    (fast, counts)
}

#[test]
fn selects_direct_property_predicates_and_projects_all_value_kinds() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let root = context
        .compile("(function(node){if(node.next)return 1;return 0;})")
        .unwrap();
    let child = runtime.test_child_function_bytecode(&root, 0).unwrap();
    assert!(
        runtime
            .test_function_exec_opcodes(&child)
            .unwrap()
            .contains(&Opcode::FieldTruthyBranch)
    );
    let negated = context
        .compile("(function(node){if(!node.next)return 1;return 0;})")
        .unwrap();
    let negated_child = runtime.test_child_function_bytecode(&negated, 0).unwrap();
    assert!(
        runtime
            .test_function_exec_opcodes(&negated_child)
            .unwrap()
            .contains(&Opcode::FieldTruthyBranch)
    );

    let source = r#"(function(){
        function decide(node){ if(node.next) return 1; return 0; }
        var values=[undefined,null,false,true,0,-0,NaN,1,-1,"","x",0n,1n,{},Symbol('s')];
        var node={next:null}, result=[];
        for(var i=0;i<values.length;i++){
            node.next=values[i];
            result.push(decide(node),decide(node));
        }
        return result.join('');
    })()"#;
    let (value, (hits, misses)) = assert_same(source);
    assert_eq!(
        value,
        Value::String(JsString::from_static("000000110000001111001100111111"))
    );
    assert!(hits > 0, "the borrowed result path must execute");
    assert!(misses > 0, "the cold shared cache must use ordinary lookup");
}

#[test]
fn negated_branch_and_loop_condition_keep_their_original_destinations() {
    let source = r#"(function(){
        function count(node){var count=0;while(node.next){count++;node=node.next;}return count;}
        function empty(node){if(!node.next)return 1;return 0;}
        var tail={next:null}, head={next:tail};
        return [count(head),count(head),empty(head),empty(tail)].join(':');
    })()"#;
    let (value, (hits, misses)) = assert_same(source);
    assert_eq!(value, Value::String(JsString::from_static("1:1:0:1")));
    assert!(hits > 0 && misses > 0);
}

#[test]
fn changing_value_shape_and_getter_preserve_branch_and_effect_order() {
    let source = r#"(function(){
        function decide(node){if(node.next)return 'T';return 'F';}
        var node={next:0}, log='';
        log+=decide(node);
        node.next={valueOf(){throw Error('ToBoolean called valueOf')}};
        log+=decide(node);
        Object.defineProperty(node,'next',{get(){log+='G';return 0;},configurable:true});
        var branch=decide(node); log+=branch;
        delete node.next;
        node.next=1;
        log+=decide(node);
        log+=decide(node);
        return log;
    })()"#;
    let (value, (hits, misses)) = assert_same(source);
    assert_eq!(value, Value::String(JsString::from_static("FTGFTT")));
    assert!(hits > 0 && misses > 0);
}

#[test]
fn proxy_missing_property_and_thrown_getter_fall_back_exactly_once() {
    let source = r#"(function(){
        function decide(node){if(node.next)return 1;return 0;}
        var log='', sentinel={};
        var accessor={};
        Object.defineProperty(accessor,'next',{get(){log+='g';throw sentinel;}});
        try {decide(accessor);} catch(error){if(error!==sentinel)throw Error('identity');}
        var proxy=new Proxy({next:1},{get(target,key,receiver){log+=String(key);return Reflect.get(target,key,receiver);}});
        var result=decide(proxy);
        var missing=decide({});
        return log+':'+result+':'+missing;
    })()"#;
    let (value, (_, misses)) = assert_same(source);
    assert_eq!(value, Value::String(JsString::from_static("gnext:1:0")));
    assert!(misses >= 3);
}

#[test]
fn self_reference_and_alternating_shapes_keep_current_location_values() {
    let source = r#"(function(){
        function decide(node){if(node.next)return 1;return 0;}
        var a={next:null}, b={tag:1,next:1}, result='';
        a.next=a;
        for(var i=0;i<12;i++){
            result+=decide(i%2?a:b);
            b.next=i%3?0:1;
        }
        return result;
    })()"#;
    let (_, (hits, misses)) = assert_same(source);
    assert!(hits > 0 && misses > 0);
}

#[test]
fn accessor_phase_recovers_to_own_data_without_replaying_effects() {
    let source = r#"(function(){
        function decide(node){if(node.next)return 1;return 0;}
        var node={next:1}, sum=0, getters=0;
        for(var i=0;i<20;i++)sum+=decide(node);
        Object.defineProperty(node,'next',{get(){getters++;return 0;},configurable:true});
        for(var i=0;i<20;i++)sum+=decide(node);
        Object.defineProperty(node,'next',{value:1,writable:true,configurable:true});
        for(var i=0;i<1100;i++)sum+=decide(node);
        return sum+':'+getters;
    })()"#;
    let (value, (hits, misses)) = assert_same(source);
    assert_eq!(value, Value::String(JsString::from_static("1120:20")));
    assert!(hits >= 20 && misses >= 20, "hits={hits} misses={misses}");
}

#[test]
fn uncertain_and_captured_bindings_keep_ordinary_behavior() {
    let tdz = r#"(function(){function f(){try{if(node.next)return false;}catch(error){return error instanceof ReferenceError;}let node;return false;}return f();})()"#;
    assert_eq!(assert_same(tdz).0, Value::Bool(true));
    let captured = r#"(function(){function outer(){var node={next:1};return function(){if(node.next)return 1;return 0;};}return outer()();})()"#;
    assert_eq!(assert_same(captured).0, Value::Int(1));
}
