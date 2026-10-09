use crate::engine::api::runtime::Runtime;
use crate::engine::api::runtime_error::RuntimeError;

use crate::engine::value::{Value, fail_next_replacement_reservation_for_test};

#[test]
fn regexp_escape_is_strict_static_generic_and_non_constructible() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    let Value::String(transcript) = context
        .eval(
            r#"
            (function () {
                var descriptor = Object.getOwnPropertyDescriptor(RegExp, "escape");
                var touched = 0;
                var inputError;
                try {
                    RegExp.escape({ toString: function () { touched++; return "x"; } });
                } catch (error) {
                    inputError = error.name + ":" + error.message;
                }
                var constructError;
                try {
                    new RegExp.escape("x");
                } catch (error) {
                    constructError = error.name;
                }
                return [
                    Object.getOwnPropertyNames(RegExp).join(","),
                    typeof RegExp.escape,
                    RegExp.escape.length,
                    RegExp.escape.name,
                    descriptor.writable,
                    descriptor.enumerable,
                    descriptor.configurable,
                    RegExp.escape.call(42, "a1_.-/"),
                    inputError,
                    touched,
                    constructError
                ].join("|");
            })()
            "#,
        )
        .expect("RegExp.escape surface probe")
    else {
        panic!("RegExp.escape surface probe did not return a String");
    };
    assert_eq!(
        transcript.to_utf8_lossy(),
        "length,name,escape,prototype|function|1|escape|true|false|true|\
         \\x611_\\.\\x2d\\/|TypeError:not a string|0|TypeError",
    );
}

#[test]
fn direct_replace_uses_a_second_buffer_while_generic_replace_keeps_the_outer_error() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    drop(
        context
            .eval("RegExp.prototype.exec")
            .expect("materialize the standard exec slot"),
    );

    fail_next_replacement_reservation_for_test();
    let Value::String(result) = context
        .eval(r#"/a/[Symbol.replace]("a","X")"#)
        .expect("the direct matcher must discard the failed outer buffer")
    else {
        panic!("direct replacement did not return a String");
    };
    assert_eq!(result.to_utf8_lossy(), "X");

    fail_next_replacement_reservation_for_test();
    assert_eq!(
        context.eval(r#"/a/[Symbol.replace]("a",function(){return "X"})"#),
        Err(RuntimeError::Exception),
        "functional replacement must keep the generic outer buffer failure",
    );
    let Value::Object(error) = context
        .take_exception()
        .expect("take replacement buffer exception")
        .expect("replacement buffer exception was missing")
    else {
        panic!("replacement buffer exception was not an Error object");
    };
    let message = runtime
        .pinned_property_key(crate::engine::atom::pinned::PinnedAtom::Message)
        .unwrap();
    let Value::String(message) = context.get_property(&error, &message).unwrap() else {
        panic!("replacement buffer Error message was not a String");
    };
    assert_eq!(message.to_utf8_lossy(), "out of memory");
}

#[test]
fn regexp_last_index_fast_paths_keep_set_semantics() {
    let runtime = Runtime::new();
    let mut context = runtime.new_context().expect("create context");
    assert_eq!(
        context
            .eval(
                r#"
            (function () {
                var out = [];
                var g = /a/g;
                out.push(g.exec("bab").index === 1 && g.lastIndex === 2);
                out.push(g.exec("bab") === null && g.lastIndex === 0);
                var y = /a/y;
                y.lastIndex = 1;
                out.push(y.exec("ba") !== null && y.lastIndex === 2);
                out.push("aXa".replace(/a/g, "b") === "bXb" && /a/g.lastIndex === 0);
                var frozen = Object.freeze(/a/g);
                try { frozen.exec("a"); out.push(false); }
                catch (e) { out.push(e instanceof TypeError && frozen.lastIndex === 0); }
                var fixed = /a/g;
                Object.defineProperty(fixed, "lastIndex", { value: 0, writable: false });
                try { "a".replace(fixed, "b"); out.push(false); }
                catch (e) { out.push(e instanceof TypeError); }
                var seen = 0;
                var sticky = /a/y;
                sticky.lastIndex = { valueOf: function () { seen++; return 1; } };
                out.push(sticky.exec("ba") !== null && seen === 1 && sticky.lastIndex === 2);
                var held = /a/g;
                held.lastIndex = "x";
                out.push(held.exec("a") !== null && held.lastIndex === 1);
                return out.every(function (x) { return x; });
            })()
        "#
            )
            .unwrap(),
        Value::Bool(true)
    );
}
