use crate::runtime_completion_oracle::observe_legacy_float_eval_completion as observe_rust_eval;
use crate::runtime_observation::host_runtime_context;

use crate::common::compile_syntax_error;

// Pins QuickJS 2026-06-04 ArrayAssignmentPattern lowering. Array binding
// declarations are covered separately: this target keeps AssignmentExpression
// result identity, Reference timing, IteratorClose, and synchronous for-in/of
// assignment heads on the assignment-specific path. Object assignment parity
// is covered by its own focused target, including recursive array/object joins.

const SMOKE_SOURCE: &str = "(function(){var answer;var source=[42];var result=([answer]=source);return answer+'|'+(result===source)})()";

#[test]
fn nested_object_assignments_run_without_an_oracle() {
    let (runtime, mut context) = host_runtime_context();
    for source in [
        "(function(){var value;({value}={value:42});return value})()",
        "(function(){var value;[{value}]=[{value:42}];return value})()",
        "(function(){var x;({p:[{x}]}={p:[{x:42}]});return x})()",
        "(function(){var x;[[{x}]]=[[{x:42}]];return x})()",
        "(function(){var x;for([{x}] of [[{x:42}]]){}return x})()",
    ] {
        assert_eq!(
            observe_rust_eval(&runtime, &mut context, source, "nested object assignment"),
            "return|number|42",
            "{source}",
        );
    }
}

#[test]
fn invalid_object_assignment_leaf_is_syntax() {
    for source in [
        "var a;({p: true && a} = {});",
        "var a,x;({p:[{x}],q:true&&a}={});",
        "'use strict';var x;({eval}={});",
    ] {
        assert_eq!(
            compile_syntax_error(source),
            "invalid destructuring target",
            "{source}"
        );
    }
}

#[test]
fn array_assignment_rest_diagnostics_have_quickjs_priority() {
    for (source, message) in [
        ("var a;[...]=[];", "missing binding pattern..."),
        (
            "var a;[...[true&&a]=[]]=[];",
            "rest element cannot have a default value",
        ),
        (
            "var a;[...{p:true&&a}=[]]=[];",
            "rest element cannot have a default value",
        ),
        (
            "var a;({p:[...[true&&a]=[]]}={});",
            "rest element cannot have a default value",
        ),
        (
            "var a;({p:[...{q:true&&a}=[]]}={});",
            "rest element cannot have a default value",
        ),
        (
            "var a,x;[...{x},true&&a]=[];",
            "rest element must be the last one",
        ),
    ] {
        assert_eq!(compile_syntax_error(source), message, "{source}");
    }
}

#[test]
fn invalid_array_assignment_leaf_is_syntax_after_object_patterns() {
    for source in [
        "var a,x;[{x},true&&a]=[];",
        "var a,x;[[{x}],true&&a]=[];",
        "var a,x;for([{x},true&&a] of []){}",
        "var a;[true&&a]=[];",
        "var a;[a()]=[];",
        "var a;[new a]=[];",
    ] {
        assert_eq!(
            compile_syntax_error(source),
            "invalid destructuring target",
            "{source}"
        );
    }
}

#[test]
fn array_assignment_smoke_runs_without_an_oracle() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        observe_rust_eval(
            &runtime,
            &mut context,
            SMOKE_SOURCE,
            "array assignment smoke"
        ),
        "return|string|42|true",
    );
}
