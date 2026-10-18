use crate::runtime_observation::string_value;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use quickjs_oxide::engine::api::core::{CallableRef, Context, JsString, Runtime, Value};

#[test]
fn foreign_realm_eval_callable_preserves_non_string_identity() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let eval = global_eval(&runtime, &mut defining);

    assert_eq!(
        caller.call(&eval, Value::Undefined, &[]).unwrap(),
        Value::Undefined
    );

    let ordinary = caller.new_object().unwrap();
    let caller_global = caller.global_object().unwrap();
    let Value::Object(returned) = caller
        .call(
            &eval,
            Value::Object(caller_global),
            &[Value::Object(ordinary.try_clone().expect("duplicate root"))],
        )
        .unwrap()
    else {
        panic!("foreign eval did not return the ordinary object");
    };
    assert_eq!(returned, ordinary);

    let Value::Object(boxed_string) = caller.eval("new String('40 + 2')").unwrap() else {
        panic!("String construction did not return an object");
    };
    let Value::Object(returned) = caller
        .call(
            &eval,
            Value::Null,
            &[Value::Object(
                boxed_string.try_clone().expect("duplicate root"),
            )],
        )
        .unwrap()
    else {
        panic!("foreign eval did not return the String object");
    };
    assert_eq!(returned, boxed_string);

    let symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("foreign eval").unwrap()))
        .unwrap();
    let Value::Symbol(returned) = caller
        .call(
            &eval,
            Value::Bool(true),
            &[Value::Symbol(symbol.try_clone().expect("duplicate root"))],
        )
        .unwrap()
    else {
        panic!("foreign eval did not return the Symbol");
    };
    assert_eq!(returned, symbol);
}

#[test]
fn syntactic_eval_replacements_take_the_complete_ordinary_call_path() {
    for (source, expected) in [
        (
            r#"
                eval = function replacement(source, extra) {
                    return source + ":" + extra + ":" + (this === globalThis);
                };
                eval("x", 7)
            "#,
            string_value("x:7:true"),
        ),
        (
            r#"
                (function (eval) {
                    return eval("x", 7);
                })(function replacement(source, extra) {
                    return source + ":" + extra + ":" + (this === globalThis);
                })
            "#,
            string_value("x:7:true"),
        ),
        (
            r#"
                eval = 1;
                try { eval("x"); "none"; } catch (error) { error.name; }
            "#,
            string_value("TypeError"),
        ),
        (
            r#"
                var trace = "";
                eval = function replacement(first, second) {
                    return trace + "|" + first + "|" + second;
                };
                eval((trace += "A", "x"), (trace += "B", 7))
            "#,
            string_value("AB|x|7"),
        ),
    ] {
        let (_runtime, mut context) = host_runtime_context();
        assert_eq!(context.eval(source).unwrap(), expected, "source: {source}");
    }

    let (_runtime, mut context) = host_runtime_context();
    assert_eq!(
        context
            .eval(
                r#"
                    var trace = "";
                    var result = eval(42, (trace = "B"));
                    result + "|" + trace
                "#
            )
            .unwrap(),
        string_value("42|B"),
        "original direct eval must evaluate every argument but consume only the first"
    );
}

#[test]
fn primitive_string_eval_executes_indirect_and_direct_completion_values() {
    let (runtime, mut context) = host_runtime_context();
    let eval = global_eval(&runtime, &mut context);

    assert_eq!(
        context
            .call(
                &eval,
                Value::Undefined,
                &[Value::String(JsString::try_from_utf8("40 + 2").unwrap())],
            )
            .unwrap(),
        Value::Int(42),
        "host Context::call did not execute primitive String eval",
    );
    assert_eq!(
        context.eval(r#"(0, eval)("40 + 2")"#).unwrap(),
        Value::Int(42)
    );
    assert_eq!(
        context.eval(r#"eval("(0, eval)('40 + 2')")"#).unwrap(),
        Value::Int(42),
        "eval source incorrectly rejected a nested indirect eval",
    );

    // Direct eval of the caller's bindings: NEWCORE-DIRECT-EVAL-001.
}

#[test]
fn eval_lexicals_are_ephemeral_but_returned_closures_retain_them() {
    for (description, source) in [
        (
            "direct",
            r#"
                (function () {
                    var closure = eval(
                        "let answer = 40; const increment = 2; " +
                        "(function () { return answer + increment; })"
                    );
                    return [closure(), typeof answer, typeof increment].join("|");
                })()
            "#,
        ),
        (
            "indirect",
            r#"
                var closure = (0, eval)(
                    "let answer = 40; const increment = 2; " +
                    "(function () { return answer + increment; })"
                );
                [closure(), typeof answer, typeof increment].join("|")
            "#,
        ),
    ] {
        let (_runtime, mut context) = host_runtime_context();
        assert_eq!(
            context.eval(source).unwrap(),
            string_value("42|undefined|undefined"),
            "{description} eval lexical lifetime drifted",
        );
    }
}

#[test]
fn eval_syntax_errors_are_catchable() {
    let (_runtime, mut context) = host_runtime_context();

    assert_eq!(
        context
            .eval(
                r#"
                    try { (0, eval)("1 +"); "none"; }
                    catch (error) { error.name; }
                "#,
            )
            .unwrap(),
        string_value("SyntaxError"),
        "indirect eval parse errors must be JavaScript exceptions",
    );
    assert_eq!(
        context
            .eval(r#"(function () { return eval("010"); })()"#)
            .unwrap(),
        Value::Int(8),
        "sloppy direct eval unexpectedly inherited strict mode",
    );
    // Direct eval inheriting the caller's strictness: NEWCORE-DIRECT-EVAL-001.
}

fn global_eval(runtime: &Runtime, context: &mut Context) -> CallableRef {
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key("eval").unwrap();
    let Value::Object(function) = context.get_property(&global, &key).unwrap() else {
        panic!("global eval was not an object");
    };
    runtime
        .as_callable(&function)
        .unwrap()
        .expect("global eval was not callable")
}
