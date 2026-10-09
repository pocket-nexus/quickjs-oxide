//! Source-level exercises for the specialized conditional-branch loop.
//!
//! These tests pin the observable behavior of the loop shapes the
//! specialized dispatch loop claims: numeric comparisons fused with
//! branches, local/argument read-write segments, unary and binary number
//! operations, stack drops, and the guard-miss fallbacks (captured and
//! non-numeric bindings). The fast path keeps no state of its own, so a
//! correct result is the whole contract; coverage of the fast set itself is
//! established by the opcode inventory and the container probe.

use crate::engine::api::{Runtime, Value};

fn eval_bool(source: &str) -> bool {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    matches!(context.eval(source).unwrap(), Value::Bool(true))
}

fn eval_number(source: &str) -> f64 {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    match context.eval(source).unwrap() {
        Value::Int(value) => f64::from(value),
        Value::Float(value) => value,
        other => panic!("expected a number, got {other:?}"),
    }
}

#[test]
fn empty_loop_shape_counts_up_and_returns_the_last_value() {
    assert_eq!(
        eval_number("function work(n){ var s; for (var i=0;i<n;i++){ s=i; } return s; }\nwork(100)"),
        99.0
    );
}

#[test]
fn float_loop_variable_walks_gte_exit_and_accumulates() {
    assert_eq!(
        eval_number("var s=0; for (var i=0.5; i<3; i+=0.5){ s+=i; } s"),
        7.5
    );
}

#[test]
fn downto_loop_with_iftrue_break_stops_at_the_marker() {
    assert_eq!(
        eval_number("var c=0; for (var i=5; i>0; i--){ c++; if (i==3) break; } c"),
        3.0
    );
}

#[test]
fn while_loop_with_not_and_postinc_value_uses_both_polarities() {
    assert!(eval_bool(
        "var i=0; var j=i++; while (!(i>=4)) { i++; } i===4 && j===0 && i++===4 && i===5"
    ));
}

#[test]
fn strict_and_loose_numeric_comparisons_match_stack_and_local_forms() {
    assert!(eval_bool(
        r#"var a=1, b=2, hits=0;
           for (var i=0; i<4; i++) {
             if (a < b) hits++;
             if (b > a) hits++;
             if (a == 1) hits++;
             if (b != 1) hits++;
             if (a === 1) hits++;
             a = a + 1; b = b - 1;
           }
           hits===7 && a===5 && b===-2"#    ));
}

#[test]
fn captured_loop_binding_falls_back_and_still_advances() {
    // `i` is captured, so the specialized store guard misses on every
    // iteration and the generic binding path must keep the loop correct.
    assert_eq!(
        eval_number("function f(){ var i=0; var g=()=>i; for (i=0; i<10; i++){} return g(); }\nf()"),
        10.0
    );
}

#[test]
fn non_numeric_local_store_falls_back_without_corrupting_operands() {
    assert!(eval_bool(
        r#"var out=[];
           for (var i=0;i<4;i++){ var s = "v" + i; out.push(s); }
           out.join(",")==="v0,v1,v2,v3""#
    ));
}

#[test]
fn lexical_loop_head_keeps_tdz_checks_on_the_fast_set_edge() {
    assert_eq!(
        eval_number("var t=0; for (let i=0; i<5; i++){ let x = i*i; t += x; } t"),
        30.0
    );
}

#[test]
fn drop_and_nip_sequences_from_expression_statements_stay_balanced() {
    assert!(eval_bool(
        r#"var s=0;
           for (var i=0;i<10;i++){ s = i; i = i; }
           var k = (1, 2, 3);
           s===9 && k===3 && s++===9"#
    ));
}

#[test]
fn unary_number_forms_cover_neg_plus_bitnot_inc_and_dec() {
    assert!(eval_bool(
        r#"var i=3, log=[];
           for (var n=0; n<3; n++){
             log.push(-i, +i, ~i, i++, --i, i--, ++i);
             i = i + 1;
           }
           log.length===21 && i===6"#
    ));
}
