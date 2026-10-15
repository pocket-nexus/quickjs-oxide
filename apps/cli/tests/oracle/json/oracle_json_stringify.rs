use crate::oracle_cases::GroupedCase as Case;
use crate::runtime_completion_oracle::rust_case_observation as rust_observation;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{RuntimeError, Value};

#[test]
fn json_stringify_keeps_deep_traversal_on_the_engine_owned_stack() {
    // Upstream QuickJS recursively consumes the host C stack here, so its
    // exact failure depth varies with compiler, ABI, and CI thread stack.
    // Keep the cross-engine vector above 256 and retain the 4096-level
    // quickjs-oxide stress as a separate, platform-independent assertion.
    let case = Case {
        group: "deep-iterative-traversal",
        description: "quickjs-oxide traverses 4096 acyclic array levels iteratively",
        source: r#"
            (function () {
                var value = 0;
                for (var index = 0; index < 4096; index++) value = [value];
                return JSON.stringify(value).length;
            })()
        "#,
        expected: "return|number|8193",
    };

    assert_eq!(rust_observation(&case), case.expected);
}

#[test]
fn json_stringify_wrapper_and_native_errors_use_the_method_defining_realm() {
    // The qjs CLI has no same-runtime multi-context bridge. This regression
    // pins the C path directly: intrinsic C functions carry their defining
    // realm, which supplies the root wrapper and native TypeError objects.
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object_prototype = defining.object_prototype().unwrap();
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let json = eval_object(&mut defining, "JSON", "defining JSON object");
    let stringify = property_callable(&runtime, &mut defining, &json, "stringify");
    let input = eval_object(&mut caller, "({ answer: 42 })", "caller input");
    let replacer = eval_object(
        &mut caller,
        r#"
            (function (key, value) {
                if (key === "") globalThis.__qjoJsonStringifyRootHolder = this;
                return value;
            })
        "#,
        "caller replacer",
    );

    let Value::String(result) = caller
        .call(
            &stringify,
            Value::Undefined,
            &[
                Value::Object(input.try_clone().expect("duplicate root")),
                Value::Object(replacer),
                Value::Undefined,
            ],
        )
        .expect("cross-realm JSON.stringify call")
    else {
        panic!("cross-realm JSON.stringify result was not a string");
    };
    assert_eq!(result.to_utf8_lossy(), r#"{"answer":42}"#);
    let root_holder = eval_object(
        &mut caller,
        "globalThis.__qjoJsonStringifyRootHolder",
        "stringify root holder",
    );
    assert_eq!(
        runtime.get_prototype_of(&root_holder).unwrap(),
        Some(defining_object_prototype),
        "JSON.stringify allocated its root holder in the callback realm",
    );
    assert_ne!(
        root_holder, input,
        "JSON.stringify exposed the input object as its root holder",
    );

    let cycle = eval_object(
        &mut caller,
        "(function () { var value = {}; value.self = value; return value; })()",
        "caller cyclic input",
    );
    assert!(matches!(
        caller.call(&stringify, Value::Undefined, &[Value::Object(cycle)],),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm JSON.stringify TypeError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error),
        "JSON.stringify allocated TypeError in the caller realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error),
        "JSON.stringify native TypeError unexpectedly used the caller realm",
    );
}
