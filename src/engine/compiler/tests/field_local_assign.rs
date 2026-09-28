use super::*;
use crate::engine::code::exec::without_field_local_assign;
use crate::engine::code::exec_opcode::Opcode;
use crate::engine::vm::test_field_assign_counts;

fn evaluate(source: &str, ordinary: bool) -> (Value, (usize, usize)) {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let (result, counts) = test_field_assign_counts(|| {
        if ordinary {
            without_field_local_assign(|| context.eval(source))
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
fn own_field_assignment_moves_a_promoted_owner_into_the_local() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context();
    let root = context
        .compile("(function(input){var node=input;node=node.next;return node.id;})")
        .unwrap();
    let child = runtime.test_child_function_bytecode(&root, 0).unwrap();
    assert!(
        runtime
            .test_function_exec_opcodes(&child)
            .unwrap()
            .contains(&Opcode::FieldLocalAssign)
    );
    let source = "(function(){function step(input){var node=input;node=node.next;return node.id;}var child={id:7},root={next:child};return step(root)+step(root);})()";
    let (value, (hits, misses)) = assert_same(source);
    assert_eq!(value, Value::Int(14));
    assert!(hits > 0);
    assert!(misses > 0);
}

#[test]
fn self_reference_and_chain_keep_the_result_alive_before_receiver_release() {
    let self_reference = "(function(){function step(input){var node=input;node=node.next;return node===input;}var root={};root.next=root;return step(root)&&step(root);})()";
    let (value, (hits, _)) = assert_same(self_reference);
    assert_eq!(value, Value::Bool(true));
    assert!(hits > 0);

    let chain = "(function(){function walk(input){var node=input;node=node.next;node=node.next;return node.id;}var tail={id:9},mid={next:tail},head={next:mid};return walk(head)+walk(head);})()";
    let (value, (hits, _)) = assert_same(chain);
    assert_eq!(value, Value::Int(18));
    assert!(hits > 0);
}

#[test]
fn scalar_destination_and_same_shape_value_changes_move_the_current_value() {
    let source = "(function(){function step(input){var out=0;out=input.next;return out.id;}var root={next:{id:1}};var a=step(root);root.next={id:5};return a+step(root)+step(root);})()";
    let (value, (hits, misses)) = assert_same(source);
    assert_eq!(value, Value::Int(11));
    assert!(hits > 0 && misses > 0);
}

#[test]
fn accessor_proxy_and_missing_field_fall_back_once() {
    let getter = "(function(){var reads=0,child={id:3},root={};Object.defineProperty(root,'next',{get(){reads++;return child;}});function step(input){var node=input;node=node.next;return node.id;}return step(root)+':'+reads;})()";
    let (value, (hits, misses)) = assert_same(getter);
    assert_eq!(value, Value::String(JsString::from_static("3:1")));
    assert_eq!(hits, 0);
    assert!(misses > 0);
    let proxy = "(function(){var reads=0,child={id:3},root=new Proxy({},{get(t,k){reads++;return child;}});function step(input){var node=input;node=node.next;return node.id;}return step(root)+':'+reads;})()";
    let (value, (hits, misses)) = assert_same(proxy);
    assert_eq!(value, Value::String(JsString::from_static("3:1")));
    assert_eq!(hits, 0);
    assert!(misses > 0);
    let missing = "(function(){function step(input){var node=input;node=node.next;return node===undefined;}return step({})&&step({});})()";
    assert_eq!(assert_same(missing).0, Value::Bool(true));
}

#[test]
fn changing_property_layout_and_thrown_identity_keep_ordinary_effects() {
    let changing = "(function(){function step(input){var node=input;node=node.next;return node.id;}var root={next:{id:1}},reads=0;var a=step(root)+step(root);Object.defineProperty(root,'next',{get(){reads++;return {id:2};}});return a+':'+step(root)+':'+reads;})()";
    let (value, (hits, misses)) = assert_same(changing);
    assert_eq!(value, Value::String(JsString::from_static("2:2:1")));
    assert!(hits > 0 && misses > 0);
    let thrown = "(function(){var error={},reads=0,root={};Object.defineProperty(root,'next',{get(){reads++;throw error;}});function step(input){var node=input;node=node.next;return node;}try{step(root);return false;}catch(e){return e===error&&reads===1;}})()";
    assert_eq!(assert_same(thrown).0, Value::Bool(true));
}

#[test]
fn captured_and_lexical_destinations_keep_ordinary_binding_rules() {
    let captured = "(function(){function step(input){var node=input;function read(){return node.id;}node=node.next;return read();}var root={next:{id:4}};return step(root);})()";
    assert_eq!(assert_same(captured).0, Value::Int(4));
    let lexical = "(function(){function step(input){let node;node=input.next;return node.id;}var root={next:{id:4}};return step(root);})()";
    assert_eq!(assert_same(lexical).0, Value::Int(4));
}
