use crate::runtime_completion_oracle::observe_legacy_float_eval_completion as observe_rust_eval;
use crate::runtime_observation::host_runtime_context;

// Pins QuickJS 2026-06-04 ObjectBindingPattern rest lowering. The excluded-key
// object is built before CopyDataProperties, and the rest target is a fresh
// ordinary object. Assignment and catch BindingPatterns have their own focused
// targets; non-simple parameters remain a separate compiler surface.

const SMOKE_SOURCE: &str = r#"(function(){
    var symbol=Symbol('keep'),source={fixed:1,other:40};source[symbol]=2;
    let {fixed,...rest}=source;
    return fixed+'|'+Object.keys(rest).join(',')+':'+rest.other+'|'+
        Object.getOwnPropertySymbols(rest).length+':'+rest[symbol];
})()"#;

#[test]
fn object_rest_smoke_runs_without_an_oracle() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        observe_rust_eval(&runtime, &mut context, SMOKE_SOURCE, "object-rest smoke"),
        "return|string|1|other:40|1:2",
    );
}
