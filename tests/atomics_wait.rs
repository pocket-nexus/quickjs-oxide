//! Timeout conversion and argument ordering at the Atomics.wait host boundary.

use quickjs_oxide::engine::api::core::{JsString, Runtime, Value};

#[test]
fn large_finite_timeouts_respect_the_non_blocking_host_policy() {
    let runtime = Runtime::new();
    runtime.set_can_block(false).unwrap();
    let context = runtime.context();
    for timeout in ["1e300", "Number.MAX_VALUE", "1e22", "Infinity", "NaN"] {
        let source = format!(
            "try {{ Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, {timeout}); }}
             catch (error) {{ error instanceof TypeError && error.message === 'cannot block in this thread'; }}"
        );
        assert_eq!(
            context.eval(&source).unwrap(),
            Value::Bool(true),
            "{timeout}"
        );
    }
}

#[test]
fn large_finite_timeouts_preserve_the_immediate_not_equal_result() {
    let runtime = Runtime::new();
    runtime.set_can_block(true).unwrap();
    let context = runtime.context();
    for (array, expected) in [("Int32Array", "1"), ("BigInt64Array", "1n")] {
        for timeout in ["1e300", "Number.MAX_VALUE", "1e22", "Infinity", "NaN"] {
            let source = format!(
                "Atomics.wait(new {array}(new SharedArrayBuffer(8)), 0, {expected}, {timeout})"
            );
            assert_eq!(
                context.eval(&source).unwrap(),
                Value::String(JsString::from_static("not-equal")),
                "{array}, {timeout}"
            );
        }
    }
}

#[test]
fn timeout_coercion_still_precedes_the_non_blocking_host_check() {
    let runtime = Runtime::new();
    runtime.set_can_block(false).unwrap();
    let context = runtime.context();
    let source = r#"
        (() => {
            const order = [];
            const index = { valueOf() { order.push('index'); return 0; } };
            const value = { valueOf() { order.push('value'); return 0; } };
            const timeout = { valueOf() { order.push('timeout'); return 1e300; } };
            try {
                Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), index, value, timeout);
            } catch (error) {
                return error instanceof TypeError && order.join(',') === 'index,value,timeout';
            }
            return false;
        })()
    "#;
    assert_eq!(context.eval(source).unwrap(), Value::Bool(true));
    let throwing = r#"
        (() => {
            const sentinel = {};
            try {
                Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0,
                    { valueOf() { throw sentinel; } });
            } catch (error) {
                return error === sentinel;
            }
            return false;
        })()
    "#;
    assert_eq!(context.eval(throwing).unwrap(), Value::Bool(true));
}

#[test]
fn non_positive_timeouts_still_expire_immediately() {
    let runtime = Runtime::new();
    runtime.set_can_block(true).unwrap();
    let context = runtime.context();
    for timeout in ["0", "-0", "-1", "-Number.MAX_VALUE", "-Infinity"] {
        let source =
            format!("Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, {timeout})");
        assert_eq!(
            context.eval(&source).unwrap(),
            Value::String(JsString::from_static("timed-out")),
            "{timeout}"
        );
    }
}
