use crate::runtime_completion_oracle::observe_legacy_float_eval_completion as observe_rust_eval;
use crate::runtime_observation::host_runtime_context;

// Pins QuickJS 2026-06-04 ObjectAssignmentPattern lowering. Object binding
// declarations are covered separately: this target keeps AssignmentExpression
// identity, full Reference timing, object-rest copying, object/array recursion,
// and synchronous for-in/of assignment heads on the assignment-specific path.

const SMOKE_SOURCE: &str = "(function(){var answer;var source={answer:42};var result=({answer}=source);return answer+'|'+(result===source)})()";

#[test]
fn object_assignment_smoke_runs_without_an_oracle() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        observe_rust_eval(
            &runtime,
            &mut context,
            SMOKE_SOURCE,
            "object assignment smoke"
        ),
        "return|string|42|true",
    );
}
