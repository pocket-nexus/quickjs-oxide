use super::support::*;
use crate::runtime_observation::host_runtime_context;

// This target pins QuickJS 2026-06-04 `JS_FlattenIntoArray` and its shared
// magic-selected `flatMap` / `flat` wrapper.

#[test]
fn array_flatten_basic_rust_smoke() {
    let (_runtime, context) = host_runtime_context();
    let value = context
        .eval(r#"[[1,2],[3,[4]],,5].flat().join("|")"#)
        .expect("evaluate basic Array.flat smoke");
    assert_eq!(primitive_value_text(value), "1|2|3|4|5");
    let value = context
        .eval(r#"[1,2,,3].flatMap(function(x,i){return [x,i]}).join("|")"#)
        .expect("evaluate basic Array.flatMap smoke");
    assert_eq!(primitive_value_text(value), "1|0|2|1|3|3");
}

#[test]
fn array_flatten_recursive_mapper_stack_overflow_is_catchable_without_oracle() {
    let (_runtime, context) = host_runtime_context();
    // A finite chain of callbacks runs to completion; an unbounded one throws
    // a catchable stack overflow.

    assert_eq!(
        primitive_value_text(
            context
                .eval(
                    r#"(function(){
            function nest(count){
                return [1].flatMap(function(value){return count?nest(count-1):[value]})
            }
            return nest(40)[0];
        })()"#
                )
                .unwrap()
        ),
        "1",
    );
    let count = { -1 };
    let value = context
        .eval(&format!(
            r#"(function(){{
                function nest(count){{
                    return [1].flatMap(function(value){{return count?nest(count-1):[value]}})
                }}
                try{{nest({count});return "missing"}}
                catch(error){{return error.name+"|"+error.message}}
            }})()"#,
        ))
        .expect("recursive Array.flatMap completion stays catchable");
    assert_eq!(primitive_value_text(value), "InternalError|stack overflow",);
    assert_eq!(primitive_value_text(context.eval("42").unwrap()), "42");
}
