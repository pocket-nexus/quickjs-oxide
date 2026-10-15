use crate::object_graph_observation::{int_property, intrinsic_prototype};
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_exception_object;
use crate::runtime_observation::{property_callable, string_property};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04 `js_string_split`
// (`quickjs.c` 45894-45980) and its prototype-table entry (46640).
//
// The JavaScript vectors intentionally stay inside quickjs-oxide's implemented
// grammar. In particular they do not use RegExp literals or constructors,
// object-literal methods/accessors, classes, arrows, destructuring, modules, or
// async syntax. RegExp integration belongs to RegExp.prototype[Symbol.split];
// this gate covers the complete generic String method, including arbitrary
// @@split delegation.

#[test]
fn string_split_cross_realm_results_errors_and_user_throws_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_string_prototype = defining.string_prototype().unwrap();
    let defining_array_prototype = defining.array_prototype().unwrap();
    let caller_array_prototype = caller.array_prototype().unwrap();
    let split = property_callable(&runtime, &mut defining, &defining_string_prototype, "split");
    assert_eq!(
        runtime.get_prototype_of(split.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    let Value::Object(result) = caller
        .call(
            &split,
            Value::String(JsString::try_from_utf8("a,b,c").unwrap()),
            &[
                Value::String(JsString::try_from_utf8(",").unwrap()),
                Value::Int(2),
            ],
        )
        .expect("cross-realm String.split call")
    else {
        panic!("cross-realm String.split result was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(defining_array_prototype),
        "String.split result did not use the method defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype),
    );
    assert_eq!(int_property(&runtime, &mut caller, &result, "length"), 2);
    assert_eq!(string_property(&runtime, &mut caller, &result, "0"), "a",);
    assert_eq!(string_property(&runtime, &mut caller, &result, "1"), "b",);

    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_eq!(
        caller.call(&split, Value::Null, &[]),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(
                &mut caller,
                "null receiver TypeError"
            ))
            .unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "String.split native TypeError did not use its defining realm",
    );

    let noncallable_separator = eval_object(
        &mut caller,
        r#"(function(){var separator=Object();separator[Symbol.split]=17;return separator})()"#,
        "caller noncallable @@split",
    );
    assert_eq!(
        caller.call(
            &split,
            Value::String(JsString::try_from_utf8("a,b").unwrap()),
            &[Value::Object(noncallable_separator)],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(
                &mut caller,
                "noncallable @@split TypeError",
            ))
            .unwrap(),
        Some(defining_type_error),
        "noncallable @@split TypeError did not use split's defining realm",
    );

    let separator = eval_object(
        &mut caller,
        r#"(function(){
            var separator=Object();
            Object.defineProperty(separator,Symbol.split,{get:function(){
                throw new TypeError("caller splitter getter");
            }});
            return separator;
        })()"#,
        "caller throwing @@split getter",
    );
    assert_eq!(
        caller.call(
            &split,
            Value::String(JsString::try_from_utf8("a,b").unwrap()),
            &[Value::Object(separator)],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(
                &mut caller,
                "caller @@split getter error"
            ))
            .unwrap(),
        Some(caller_type_error),
        "String.split replaced a user throw with a defining-realm error",
    );
}

#[test]
fn detached_string_split_and_result_retain_then_release_their_defining_realm() {
    let runtime = host_runtime();
    let split = {
        let mut defining = runtime.new_context().expect("create context");
        let prototype = defining.string_prototype().unwrap();
        property_callable(&runtime, &mut defining, &prototype, "split")
    };
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1,
        "detached split must retain its defining realm",
    );

    let result = {
        let mut caller = runtime.new_context().expect("create context");
        let Value::Object(result) = caller
            .call(
                &split,
                Value::String(JsString::try_from_utf8("x-y").unwrap()),
                &[Value::String(JsString::try_from_utf8("-").unwrap())],
            )
            .expect("detached split call")
        else {
            panic!("detached split result was not an object");
        };
        assert_eq!(int_property(&runtime, &mut caller, &result, "length"), 2);
        result
    };
    drop(split);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1,
        "split result must retain its defining Array realm",
    );
    drop(result);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").live,
        0,
        "split callable, result, and defining realm must be collectable",
    );
}
