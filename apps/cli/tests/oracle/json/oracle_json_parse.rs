use crate::object_graph_observation::object_property;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

#[test]
fn json_parse_allocations_and_native_errors_use_the_method_defining_realm() {
    // The qjs CLI has no same-runtime multi-context bridge, so this regression
    // pins the C path directly: QuickJS stores a realm on every intrinsic C
    // function (`JS_NewCFunction3`) and switches to it before `js_json_parse`
    // allocates the parse graph, reviver contexts, wrapper, or SyntaxError.
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_object_prototype = defining.object_prototype().unwrap();
    let defining_array_prototype = defining.array_prototype().unwrap();
    let defining_syntax_error = eval_object(
        &mut defining,
        "SyntaxError.prototype",
        "defining SyntaxError prototype",
    );
    let caller_syntax_error = eval_object(
        &mut caller,
        "SyntaxError.prototype",
        "caller SyntaxError prototype",
    );
    let json = eval_object(&mut defining, "JSON", "defining JSON object");
    let parse = property_callable(&runtime, &mut defining, &json, "parse");
    let reviver_object = eval_object(
        &mut caller,
        r#"
            (function (key, value, context) {
                if (key === "answer") {
                    globalThis.__qjoJsonParseContext = context;
                    globalThis.__qjoJsonParseAnswerHolder = this;
                }
                if (key === "") globalThis.__qjoJsonParseRootHolder = this;
                return value;
            })
        "#,
        "caller reviver",
    );

    let Value::Object(result) = caller
        .call(
            &parse,
            Value::Undefined,
            &[
                Value::String(JsString::try_from_utf8(r#"{"answer":42,"array":[1]}"#).unwrap()),
                Value::Object(reviver_object),
            ],
        )
        .expect("cross-realm JSON.parse call")
    else {
        panic!("cross-realm JSON.parse result was not an object");
    };
    let array = object_property(&runtime, &mut caller, &result, "array");
    let context = eval_object(
        &mut caller,
        "globalThis.__qjoJsonParseContext",
        "reviver context",
    );
    let answer_holder = eval_object(
        &mut caller,
        "globalThis.__qjoJsonParseAnswerHolder",
        "answer holder",
    );
    let root_holder = eval_object(
        &mut caller,
        "globalThis.__qjoJsonParseRootHolder",
        "root holder",
    );

    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_object_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "JSON.parse allocated its object graph in the caller realm",
    );
    assert_eq!(
        runtime.get_prototype_of(&array).unwrap(),
        Some(defining_array_prototype),
        "JSON.parse allocated its arrays in the caller realm",
    );
    assert_eq!(
        runtime.get_prototype_of(&context).unwrap(),
        Some(
            defining_object_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "JSON.parse allocated reviver contexts in the callback realm",
    );
    assert_eq!(
        answer_holder, result,
        "reviver received the wrong child holder"
    );
    assert_ne!(
        root_holder, result,
        "JSON.parse exposed the result as its root holder"
    );
    assert_eq!(
        runtime.get_prototype_of(&root_holder).unwrap(),
        Some(defining_object_prototype),
        "JSON.parse allocated its root holder in the callback realm",
    );

    assert!(matches!(
        caller.call(
            &parse,
            Value::Undefined,
            &[Value::String(JsString::try_from_utf8("{").unwrap())],
        ),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm JSON.parse SyntaxError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_syntax_error),
        "JSON.parse allocated SyntaxError in the caller realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_syntax_error),
        "JSON.parse native SyntaxError unexpectedly used the caller realm",
    );
}
