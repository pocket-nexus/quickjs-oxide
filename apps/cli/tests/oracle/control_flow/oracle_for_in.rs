use crate::runtime_observation::host_runtime_context;

use quickjs_oxide::engine::api::core::Value;

#[test]
fn for_in_rust_smoke_runs_without_an_oracle() {
    let (_runtime, mut context) = host_runtime_context();
    let Value::String(value) = context
        .eval("(function(){var s='';for(var k in {b:1,a:1})s+=k;return s})()")
        .expect("execute for-in smoke")
    else {
        panic!("for-in smoke did not return a String");
    };
    assert_eq!(value.to_utf8_lossy(), "ba");
}
