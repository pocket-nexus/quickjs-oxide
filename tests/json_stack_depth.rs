//! JSON nesting guard.
//!
//! Deeply nested JSON must surface a catchable `stack overflow` instead of
//! overflowing the host stack. `JSON.parse` recurses once per open container
//! against a logical budget matching QuickJS 2026-06-04 at top level, backed
//! by a physical host-stack check; the reviver walk keeps its nodes on the
//! heap under its own budget. The boundary tests run on an explicitly sized
//! thread, as the `qjs` CLI does.

use quickjs_oxide::engine::api::core::{Context, Runtime, Value};
use quickjs_oxide_host::SystemHost;

/// Host stack for the boundary thread. Debug frames are larger than release
/// frames. The reservation is virtual address space, not RSS.
#[cfg(debug_assertions)]
const TEST_STACK_SIZE: usize = 256 * 1024 * 1024;
#[cfg(not(debug_assertions))]
const TEST_STACK_SIZE: usize = 32 * 1024 * 1024;

const PARSE_DEPTH: usize = 10_891;
const REVIVER_DEPTH: usize = 3_630;

fn run_on_stack<T: Send + 'static>(
    size: usize,
    declare: bool,
    task: impl FnOnce() -> T + Send + 'static,
) -> T {
    std::thread::Builder::new()
        .name("json-stack-depth".to_owned())
        .stack_size(size)
        .spawn(move || {
            if declare {
                quickjs_oxide::engine::api::core::declare_thread_stack(size);
            }
            task()
        })
        .unwrap()
        .join()
        .unwrap()
}

fn new_context() -> (Runtime, Context) {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let context = runtime.new_context().expect("create context");
    (runtime, context)
}

/// Run `JSON.parse` on `open * depth + leaf + close * depth` and report `OK`
/// or the caught error's name, message and column.
fn parse_outcome(
    context: &mut Context,
    open: &str,
    leaf: &str,
    close: &str,
    depth: usize,
    reviver: &str,
) -> String {
    let source = format!(
        r#"
        (() => {{
            const text = {open:?}.repeat({depth}) + {leaf:?} + {close:?}.repeat({depth});
            try {{
                JSON.parse(text{reviver});
                return "OK";
            }} catch (error) {{
                return error.name + ":" + error.message + "@" + error.columnNumber;
            }}
        }})()
        "#
    );
    match context.eval(&source) {
        Ok(Value::String(value)) => value.to_utf8_lossy(),
        Ok(other) => panic!("expected a string completion, got {other:?}"),
        Err(error) => panic!("evaluation failed: {error}"),
    }
}

#[test]
fn parse_and_reviver_budgets_throw_catchable_errors() {
    run_on_stack(TEST_STACK_SIZE, true, || {
        let (_runtime, mut context) = new_context();
        let mut outcome = |open, leaf, close, depth, reviver| {
            parse_outcome(&mut context, open, leaf, close, depth, reviver)
        };
        let reviver = ", function (key, value) { return value; }";

        assert_eq!(outcome("[", "0", "]", PARSE_DEPTH, ""), "OK");
        assert_eq!(
            outcome("[", "0", "]", PARSE_DEPTH + 1, ""),
            "SyntaxError:stack overflow@10893"
        );
        // An empty innermost container is one value shallower.
        assert_eq!(outcome("[", "", "]", PARSE_DEPTH + 1, ""), "OK");
        assert_eq!(
            outcome("[", "", "]", PARSE_DEPTH + 2, ""),
            "SyntaxError:stack overflow@10893"
        );
        assert_eq!(outcome("{\"a\":", "0", "}", PARSE_DEPTH, ""), "OK");
        assert_eq!(
            outcome("{\"a\": ", "0", "}", PARSE_DEPTH + 1, ""),
            "SyntaxError:stack overflow@65353"
        );

        assert_eq!(outcome("[", "0", "]", REVIVER_DEPTH, reviver), "OK");
        assert_eq!(
            outcome("[", "0", "]", REVIVER_DEPTH + 1, reviver),
            "InternalError:stack overflow@undefined"
        );
    });
}

#[test]
fn deep_json_never_aborts_and_the_runtime_recovers() {
    for (size, declare) in [(TEST_STACK_SIZE, true), (1024 * 1024, false)] {
        run_on_stack(size, declare, move || {
            let (_runtime, mut context) = new_context();
            // Far beyond the budget, and on a thread too small to reach it,
            // the error is catchable rather than a host stack overflow.
            for depth in [PARSE_DEPTH + 1, 1_000_000] {
                let thrown = parse_outcome(&mut context, "[", "0", "]", depth, "");
                assert!(
                    thrown.starts_with("SyntaxError:stack overflow@"),
                    "depth {depth} on a {size}-byte thread: {thrown}"
                );
            }
            assert_eq!(
                parse_outcome(&mut context, "[", "0", "]", 100, ""),
                "OK",
                "shallow JSON after an overflow on a {size}-byte thread"
            );
        });
    }
}
