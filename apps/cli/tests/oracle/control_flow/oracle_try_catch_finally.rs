use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::Value;

#[test]
fn try_catch_cross_realm_regression() {
    // Pinned with the QuickJS C API compile-in-A/execute-in-B path. The Rust
    // API exposes that operation directly; QJS_ORACLE keeps this test grouped
    // with the same checksum-pinned release as the process differentials.
    let _oracle = require_qjs_oracle!("try/catch cross-realm oracle");

    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let caught_error = defining.compile("try{null.realmFault}catch(e){e}").unwrap();
    let Value::Object(caught_error) = caller.execute(&caught_error).unwrap() else {
        panic!("cross-realm catch did not return its native Error object");
    };
    let Value::Object(type_error_a) = defining.eval("TypeError.prototype").unwrap() else {
        panic!("defining TypeError.prototype was not an object");
    };
    let Value::Object(type_error_b) = caller.eval("TypeError.prototype").unwrap() else {
        panic!("caller TypeError.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&caught_error).unwrap(),
        Some(type_error_a)
    );
    assert_ne!(
        runtime.get_prototype_of(&caught_error).unwrap(),
        Some(type_error_b)
    );

    let caught_object = defining.compile("try{throw Function}catch(e){e}").unwrap();
    let caught_object = caller.execute(&caught_object).unwrap();
    let caller_function = caller.eval("Function").unwrap();
    let defining_function = defining.eval("Function").unwrap();
    assert_eq!(caught_object, caller_function);
    assert_ne!(caught_object, defining_function);

    let caught_closure = defining
        .compile("try{throw 5}catch(e){(function(){return e})}")
        .unwrap();
    let Value::Object(caught_closure) = caller.execute(&caught_closure).unwrap() else {
        panic!("cross-realm catch did not return its captured closure");
    };
    let Value::Object(function_prototype_a) = defining.eval("Function.prototype").unwrap() else {
        panic!("defining Function.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&caught_closure).unwrap(),
        Some(function_prototype_a)
    );
    let caught_closure = runtime.as_callable(&caught_closure).unwrap().unwrap();
    assert_eq!(
        caller.call(&caught_closure, Value::Undefined, &[]).unwrap(),
        Value::Int(5)
    );
}
