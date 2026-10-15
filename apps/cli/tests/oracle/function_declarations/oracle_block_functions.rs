use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::{JsString, Value};

#[test]
fn block_function_cross_realm_regression() {
    // Pinned with the QuickJS C API compile-in-A/execute-in-B path. The Rust
    // API exposes the same operation directly, so this remains unconditional.
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    drop(defining.eval("globalThis.realmTag='A'").unwrap());
    drop(caller.eval("globalThis.realmTag='B'").unwrap());

    let bytecode = defining
        .compile(
            "{function crossRealmBlock(){return realmTag+'|'+(this===globalThis)}\
             Function.savedCrossRealmBlock=crossRealmBlock}\
             crossRealmBlock",
        )
        .unwrap();
    let Value::Object(outer) = caller.execute(&bytecode).unwrap() else {
        panic!("cross-realm block Program did not return its Annex function");
    };
    let Value::Object(inner) = caller.eval("Function.savedCrossRealmBlock").unwrap() else {
        panic!("cross-realm block did not save its lexical function");
    };
    assert_ne!(outer, inner);

    let Value::Object(prototype_a) = defining.eval("Function.prototype").unwrap() else {
        panic!("defining Function.prototype was not an object");
    };
    let Value::Object(prototype_b) = caller.eval("Function.prototype").unwrap() else {
        panic!("caller Function.prototype was not an object");
    };
    for function in [&outer, &inner] {
        assert_eq!(
            runtime.get_prototype_of(function).unwrap(),
            Some(prototype_a.try_clone().expect("duplicate root"))
        );
        assert_ne!(
            runtime.get_prototype_of(function).unwrap(),
            Some(prototype_b.try_clone().expect("duplicate root"))
        );
        let callable = runtime.as_callable(function).unwrap().unwrap();
        assert_eq!(
            caller.call(&callable, Value::Undefined, &[]).unwrap(),
            Value::String(JsString::try_from_utf8("B|false").unwrap())
        );
    }
}
