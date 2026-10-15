use crate::runtime_observation::host_runtime;
use crate::runtime_observation::primitive_value_text_with_rust_float as primitive_value_text;

use crate::runtime_oracle::error_string_property;
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

#[test]
fn function_body_declaration_cross_realm_regression() {
    // These expectations are pinned from the QuickJS C API's compile-in-A,
    // execute-in-B path; the Rust API exposes the same two-context operation
    // directly, so keep it in the unconditional product regression suite.
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    drop(defining.eval("globalThis.realmTag='A'").unwrap());
    drop(caller.eval("globalThis.realmTag='B'").unwrap());

    let bytecode = defining
        .compile(
            "(function crossRealmOuter(value,selectBoom){\
                 function crossRealmInner(){\
                   return value+'|'+realmTag+'|'+\
                     (this===globalThis)\
                 }\
                 function crossRealmBoom(){missingBodyRealm}\
                 if(selectBoom)return crossRealmBoom;\
                 return crossRealmInner\
             })",
        )
        .unwrap();
    let Value::Object(outer) = caller.execute(&bytecode).unwrap() else {
        panic!("cross-realm Program did not return its outer function");
    };

    let Value::Object(function_prototype_a) = defining.eval("Function.prototype").unwrap() else {
        panic!("defining Function.prototype was not an object");
    };
    let Value::Object(function_prototype_b) = caller.eval("Function.prototype").unwrap() else {
        panic!("caller Function.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&outer).unwrap(),
        Some(function_prototype_a.try_clone().expect("duplicate root"))
    );
    assert_ne!(
        runtime.get_prototype_of(&outer).unwrap(),
        Some(function_prototype_b.try_clone().expect("duplicate root"))
    );

    let outer = runtime.as_callable(&outer).unwrap().unwrap();
    let Value::Object(inner) = caller
        .call(
            &outer,
            Value::Undefined,
            &[
                Value::String(JsString::try_from_utf8("argB").unwrap()),
                Value::Bool(false),
            ],
        )
        .unwrap()
    else {
        panic!("outer function did not return its inner declaration");
    };
    assert_eq!(
        runtime.get_prototype_of(&inner).unwrap(),
        Some(function_prototype_a.try_clone().expect("duplicate root"))
    );
    assert_ne!(
        runtime.get_prototype_of(&inner).unwrap(),
        Some(function_prototype_b)
    );
    let inner = runtime.as_callable(&inner).unwrap().unwrap();
    let inner_result = caller
        .call(&inner, Value::Undefined, &[])
        .unwrap_or_else(|failure| {
            let exception = caller
                .take_exception()
                .expect("take cross-realm inner exception")
                .expect("cross-realm inner exception was missing");
            let detail = match exception {
                Value::Object(error) => format!(
                    "{}: {}",
                    error_string_property(
                        &runtime,
                        &mut caller,
                        &error,
                        "name",
                        "cross-realm inner call",
                    ),
                    error_string_property(
                        &runtime,
                        &mut caller,
                        &error,
                        "message",
                        "cross-realm inner call",
                    ),
                ),
                value => primitive_value_text(value),
            };
            panic!("cross-realm inner call failed with {failure}: {detail}",);
        });
    assert_eq!(
        inner_result,
        Value::String(JsString::try_from_utf8("argB|B|false").unwrap())
    );

    let Value::Object(boom) = caller
        .call(
            &outer,
            Value::Undefined,
            &[
                Value::String(JsString::try_from_utf8("argB").unwrap()),
                Value::Bool(true),
            ],
        )
        .unwrap()
    else {
        panic!("outer function did not return its failing declaration");
    };
    let boom = runtime.as_callable(&boom).unwrap().unwrap();
    assert_eq!(
        caller.call(&boom, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    );
    let Value::Object(error) = caller
        .take_exception()
        .unwrap()
        .expect("cross-realm body declaration exception")
    else {
        panic!("cross-realm body declaration did not throw an object");
    };
    let Value::Object(reference_error_prototype_a) =
        defining.eval("ReferenceError.prototype").unwrap()
    else {
        panic!("defining ReferenceError.prototype was not an object");
    };
    let Value::Object(reference_error_prototype_b) =
        caller.eval("ReferenceError.prototype").unwrap()
    else {
        panic!("caller ReferenceError.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(reference_error_prototype_a)
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(reference_error_prototype_b)
    );
}
