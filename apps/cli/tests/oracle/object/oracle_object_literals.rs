use crate::runtime_observation::host_runtime;
use quickjs_oxide::engine::api::core::Value;

#[test]
fn object_literal_result_uses_the_defining_realm() {
    let runtime = host_runtime();
    let defining = runtime.new_context().expect("create context");
    let caller = runtime.new_context().expect("create context");
    let function = defining
        .eval("(function(){return {marker:1}})")
        .expect("object-literal factory compilation failed");
    let Value::Object(function) = function else {
        panic!("factory was not an Object");
    };
    let callable = runtime
        .as_callable(&function)
        .expect("callability lookup failed")
        .expect("factory was not callable");
    let Value::Object(result) = caller
        .call(&callable, Value::Undefined, &[])
        .expect("cross-realm object-literal call failed")
    else {
        panic!("factory result was not an Object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(defining.object_prototype().unwrap())
    );
}
