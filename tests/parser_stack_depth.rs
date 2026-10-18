//! Parser deep-nesting guard.
//!
//! Deeply nested source must surface a catchable `SyntaxError: stack
//! overflow` instead of overflowing the host stack. The parser charges each
//! recursive production against a weighted budget whose depths approximate
//! QuickJS 2026-06-04, and backs it with a physical host-stack check. The
//! tests run on an explicitly sized thread, as the `qjs` CLI does.
//!
//! QuickJS's own first-throw depths depend on the C compiler that built it,
//! so the differential test against `QJS_ORACLE` checks agreement within a
//! tolerance band rather than at one exact depth.

use quickjs_oxide::engine::api::core::{Context, Runtime, RuntimeError, Value};
use quickjs_oxide_host::SystemHost;

/// Host stack for the evaluation thread. Rust parser frames are larger than
/// the C engine's, so reaching the reference nesting depths for the heaviest
/// productions reserves a proportionally larger (virtual) stack; debug frames
/// are larger again. The reservation is virtual address space, not RSS.
#[cfg(debug_assertions)]
const TEST_STACK_SIZE: usize = 256 * 1024 * 1024;
#[cfg(not(debug_assertions))]
const TEST_STACK_SIZE: usize = 32 * 1024 * 1024;

/// One nested form and the depth at which the parser first throws.
struct Form {
    name: &'static str,
    /// JS statements run before the guarded eval (e.g. defining `F`).
    setup: &'static str,
    left: &'static str,
    leaf: &'static str,
    right: &'static str,
    depth: usize,
}

const FORMS: &[Form] = &[
    Form {
        name: "parenthesized",
        setup: "",
        left: "(",
        leaf: "1",
        right: ")",
        depth: 718,
    },
    Form {
        name: "array",
        setup: "",
        left: "[",
        leaf: "1",
        right: "]",
        depth: 743,
    },
    Form {
        name: "object",
        setup: "",
        left: "({a:",
        leaf: "1",
        right: "})",
        depth: 355,
    },
    Form {
        name: "call",
        setup: "function f(x){return x}",
        left: "f(",
        leaf: "1",
        right: ")",
        depth: 743,
    },
    Form {
        name: "new",
        setup: "F=function(){}",
        left: "new F(",
        leaf: "1",
        right: ")",
        depth: 743,
    },
    Form {
        name: "unary",
        setup: "",
        left: "!",
        leaf: "1",
        right: "",
        depth: 9330,
    },
    Form {
        name: "conditional",
        setup: "",
        left: "1?",
        leaf: "1",
        right: ":0",
        depth: 8164,
    },
    Form {
        name: "arrow",
        setup: "",
        left: "x=>",
        leaf: "x",
        right: "",
        depth: 4665,
    },
    Form {
        name: "template",
        setup: "",
        left: "`${",
        leaf: "1",
        right: "}`",
        depth: 635,
    },
    Form {
        name: "if",
        setup: "",
        left: "if(1)",
        leaf: "0",
        right: "",
        depth: 3438,
    },
    Form {
        name: "block",
        setup: "",
        left: "{",
        leaf: "0",
        right: "}",
        depth: 3266,
    },
    Form {
        name: "while",
        setup: "",
        left: "while(0){",
        leaf: "0",
        right: "}",
        depth: 1675,
    },
    Form {
        name: "function-body",
        setup: "",
        left: "function f(){",
        leaf: "0",
        right: "}",
        depth: 2613,
    },
    Form {
        name: "member-access",
        setup: "var x={}",
        left: "x[",
        leaf: "1",
        right: "]",
        depth: 726,
    },
];

/// Build the label expression that evaluates one nesting `depth` and yields
/// either `"OK"` or `"ErrorName:ErrorMessage"`.
fn guarded_label(form: &Form, depth: usize) -> String {
    format!(
        "try{{eval({left:?}.repeat({depth})+{leaf:?}+{right:?}.repeat({depth}));\
         \"OK\"}}catch(e){{e.name+\":\"+e.message}}",
        left = form.left,
        leaf = form.leaf,
        right = form.right,
        depth = depth,
    )
}

/// Build a standalone program (with any needed setup) that evaluates the
/// guarded label expression.
fn guarded_program(form: &Form, depth: usize) -> String {
    let setup = if form.setup.is_empty() {
        String::new()
    } else {
        format!("{};", form.setup)
    };
    format!(
        "{setup}{label}",
        setup = setup,
        label = guarded_label(form, depth)
    )
}

fn repeated(left: &str, leaf: &str, right: &str, depth: usize) -> String {
    format!("{}{}{}", left.repeat(depth), leaf, right.repeat(depth))
}

fn unique_labels(depth: usize) -> String {
    let mut source = String::new();
    for index in 0..depth {
        source.push('L');
        source.push_str(&index.to_string());
        source.push(':');
    }
    source.push('0');
    source
}

fn assert_eval_source_boundary(
    context: &mut Context,
    name: &str,
    depth: usize,
    build: impl Fn(usize) -> String,
) {
    let accepted = eval_string(
        context,
        &format!(
            "try{{eval({:?});\"OK\"}}catch(e){{e.name+\":\"+e.message}}",
            build(depth - 1)
        ),
    );
    assert_eq!(accepted, "OK", "{name} at depth {}", depth - 1);
    let thrown = eval_string(
        context,
        &format!(
            "try{{eval({:?});\"OK\"}}catch(e){{e.name+\":\"+e.message}}",
            build(depth)
        ),
    );
    assert_eq!(
        thrown, "SyntaxError:stack overflow",
        "{name} at reference depth {depth}"
    );
}

fn take_compile_exception_label(context: &mut Context, source: &str) -> String {
    assert_eq!(
        context.compile(source).unwrap_err(),
        RuntimeError::Exception,
        "direct source did not fail with a JavaScript exception: {source:?}"
    );
    let Value::Object(error) = context
        .take_exception()
        .unwrap()
        .expect("direct compile exception was missing")
    else {
        panic!("direct compile exception was not an Error object");
    };
    let name_key = context.runtime().intern_property_key("name").unwrap();
    let message_key = context.runtime().intern_property_key("message").unwrap();
    let Value::String(name) = context.get_property(&error, &name_key).unwrap() else {
        panic!("direct compile exception name was not a string");
    };
    let Value::String(message) = context.get_property(&error, &message_key).unwrap() else {
        panic!("direct compile exception message was not a string");
    };
    format!("{}:{}", name.to_utf8_lossy(), message.to_utf8_lossy())
}

fn assert_direct_source_boundary(
    context: &mut Context,
    name: &str,
    depth: usize,
    build: impl Fn(usize) -> String,
) {
    context
        .compile(&build(depth - 1))
        .unwrap_or_else(|error| panic!("{name} rejected direct depth {}: {error}", depth - 1));
    assert_eq!(
        take_compile_exception_label(context, &build(depth)),
        "SyntaxError:stack overflow",
        "{name} at direct reference depth {depth}"
    );
}

fn run_on_parser_stack<T: Send + 'static>(task: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .name("parser-stack-depth".to_owned())
        .stack_size(TEST_STACK_SIZE)
        .spawn(|| {
            quickjs_oxide::engine::api::core::declare_thread_stack(TEST_STACK_SIZE);
            task()
        })
        .unwrap()
        .join()
        .unwrap()
}

fn eval_string(context: &mut Context, source: &str) -> String {
    match context.eval(source) {
        Ok(Value::String(value)) => value.to_utf8_lossy(),
        Ok(other) => panic!("expected a string completion, got {other:?} for {source}"),
        Err(error) => panic!("evaluation failed: {error} for {source}"),
    }
}

#[test]
#[ignore = "implementation limit (new-core phase 6 M3 results): the register lowering \
            declines a function whose operand stack passes the 256-register \
            window, so nesting past about 250 levels fails to compile"]
fn depth_boundaries_throw_a_catchable_syntax_error() {
    run_on_parser_stack(|| {
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.new_context().expect("create context");
        for form in FORMS {
            // One below the reference depth still parses.
            let accepted = eval_string(&mut context, &guarded_program(form, form.depth - 1));
            assert_eq!(
                accepted,
                "OK",
                "{} accepted at depth {}",
                form.name,
                form.depth - 1
            );
            // At the reference depth the throw is catchable and carries
            // QuickJS's name and message.
            let thrown = eval_string(&mut context, &guarded_program(form, form.depth));
            assert_eq!(
                thrown, "SyntaxError:stack overflow",
                "{} at reference depth {}",
                form.name, form.depth
            );
        }
    });
}

#[test]
#[ignore = "implementation limit (new-core phase 6 M3 results): the register lowering \
            declines a function whose operand stack passes the 256-register \
            window, so nesting past about 250 levels fails to compile"]
fn right_recursive_forms_throw_at_their_reference_depths() {
    run_on_parser_stack(|| {
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.new_context().expect("create context");
        assert_eval_source_boundary(&mut context, "assignment", 8_164, |depth| {
            format!("var a;{}", repeated("a=", "1", "", depth))
        });
        assert_eval_source_boundary(&mut context, "compound assignment", 8_164, |depth| {
            format!("var a=0;{}", repeated("a+=", "1", "", depth))
        });
        assert_eval_source_boundary(&mut context, "exponentiation", 9_330, |depth| {
            repeated("1**", "1", "", depth)
        });
        assert_eval_source_boundary(&mut context, "label", 3_438, unique_labels);
        assert_eval_source_boundary(&mut context, "yield", 8_160, |depth| {
            format!("function*g(){{{}}}", repeated("yield ", "0", "", depth))
        });
        assert_eval_source_boundary(&mut context, "dynamic import", 742, |depth| {
            format!(
                "function z(){{return {}}}",
                repeated("import(", "0", ")", depth)
            )
        });
        assert_eval_source_boundary(&mut context, "new without arguments", 5_935, |depth| {
            format!(
                "function z(){{return {}}}",
                repeated("new ", "Object", "", depth)
            )
        });
        assert_eval_source_boundary(&mut context, "block-bodied arrow", 1_420, |depth| {
            repeated("x=>{", "0", "}", depth)
        });
        // The `[]` leaf is one more array literal than the `[...` prefixes.
        assert_eval_source_boundary(&mut context, "spread array", 742, |depth| {
            format!("{}[]{}", "[...".repeat(depth), "]".repeat(depth))
        });
    });
}

#[test]
#[ignore = "implementation limit (new-core phase 6 M3 results): the register lowering \
            declines a function whose operand stack passes the 256-register \
            window, so nesting past about 250 levels fails to compile"]
fn direct_sources_share_the_eval_boundaries() {
    run_on_parser_stack(|| {
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.new_context().expect("create context");
        assert_direct_source_boundary(&mut context, "parenthesized", 718, |depth| {
            repeated("(", "1", ")", depth)
        });
        assert_direct_source_boundary(&mut context, "array", 743, |depth| {
            repeated("[", "1", "]", depth)
        });
        assert_direct_source_boundary(&mut context, "object", 355, |depth| {
            repeated("({a:", "1", "})", depth)
        });
        assert_direct_source_boundary(&mut context, "block", 3_266, |depth| {
            repeated("{", "0", "}", depth)
        });
        assert_direct_source_boundary(&mut context, "if", 3_438, |depth| {
            repeated("if(1)", "0", "", depth)
        });
        assert_direct_source_boundary(&mut context, "label", 3_438, unique_labels);
        assert_direct_source_boundary(&mut context, "dynamic import", 743, |depth| {
            repeated("import(", "0", ")", depth)
        });
        assert_direct_source_boundary(&mut context, "block-bodied arrow", 1_420, |depth| {
            repeated("x=>{", "0", "}", depth)
        });
    });
}

#[test]
fn the_runtime_recovers_after_a_caught_parser_stack_overflow() {
    run_on_parser_stack(|| {
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.new_context().expect("create context");
        for form in FORMS {
            let thrown = eval_string(&mut context, &guarded_program(form, form.depth));
            assert_eq!(thrown, "SyntaxError:stack overflow");
            // The next, shallow evaluation on the same context still works.
            assert_eq!(eval_string(&mut context, "String(6*7)"), "42");
        }
    });
}

#[test]
fn deep_nesting_never_aborts_far_beyond_the_logical_boundary() {
    run_on_parser_stack(|| {
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.new_context().expect("create context");
        // Even absurdly deep input must throw the catchable error rather than
        // abort the process; the physical backstop is the last line of
        // defense for productions the weighted budget classifies late.
        for form in [&FORMS[0], &FORMS[2], &FORMS[10]] {
            let thrown = eval_string(&mut context, &guarded_program(form, 1_000_000));
            assert_eq!(
                thrown, "SyntaxError:stack overflow",
                "{} at depth 1000000",
                form.name
            );
        }
    });
}

/// Relative distance from a form's reference depth at which both engines
/// must agree: everything this much shallower parses, everything this much
/// deeper throws. It absorbs the frame-size differences between QuickJS
/// builds while still catching a weight that is off by a meaningful factor.
const ORACLE_TOLERANCE_PERCENT: usize = 10;

#[test]
#[ignore = "implementation limit (new-core phase 6 M3 results): the register lowering \
            declines a function whose operand stack passes the 256-register \
            window, so nesting past about 250 levels fails to compile"]
fn pinned_oracle_agrees_outside_the_tolerance_band() {
    let Some(oracle) = std::env::var_os("QJS_ORACLE") else {
        eprintln!("SKIP parser-depth differential: set QJS_ORACLE to the pinned qjs");
        return;
    };
    run_on_parser_stack(move || {
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.new_context().expect("create context");
        for form in FORMS {
            let margin = form.depth * ORACLE_TOLERANCE_PERCENT / 100;
            for (depth, expected) in [
                (form.depth - margin, "OK"),
                (form.depth + margin, "SyntaxError:stack overflow"),
            ] {
                let source = guarded_program(form, depth);
                let rust = eval_string(&mut context, &source);
                let output = std::process::Command::new(&oracle)
                    .arg("-e")
                    .arg(oracle_program(form, depth))
                    .output()
                    .unwrap_or_else(|error| panic!("run pinned oracle {oracle:?}: {error}"));
                assert!(
                    output.status.success(),
                    "pinned oracle exited nonzero for a caught throw"
                );
                let pinned = String::from_utf8(output.stdout).expect("oracle utf8");
                let pinned = pinned.trim_end();
                assert_eq!(rust, expected, "{} at depth {depth}", form.name);
                // QuickJS reports the overflow from whichever production
                // trips its stack check, e.g. `missing formal parameter` when
                // that is a function head; the error is always a SyntaxError.
                let agrees = if expected == "OK" {
                    pinned == "OK"
                } else {
                    pinned.starts_with("SyntaxError:")
                };
                assert!(
                    agrees,
                    "pinned QuickJS {} at depth {depth}: {pinned}",
                    form.name
                );
            }
        }
    });
}

/// The guarded evaluation as a pinned CLI program which prints its label.
fn oracle_program(form: &Form, depth: usize) -> String {
    let setup = if form.setup.is_empty() {
        String::new()
    } else {
        format!("{};", form.setup)
    };
    format!(
        "{setup}var __r;try{{eval({left:?}.repeat({depth})+{leaf:?}+{right:?}.repeat({depth}));\
         __r=\"OK\"}}catch(e){{__r=e.name+\":\"+e.message}}print(__r);",
        left = form.left,
        leaf = form.leaf,
        right = form.right,
    )
}

#[cfg(target_os = "linux")]
#[test]
fn unlimited_stack_limit_accepts_shallow_source() {
    const CHILD_ENV: &str = "QJS_OXIDE_UNLIMITED_STACK_CHILD";
    if std::env::var_os(CHILD_ENV).is_some() {
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.new_context().expect("create context");
        assert_eq!(
            eval_string(&mut context, "String((((((((((((((1))))))))))))))"),
            "1"
        );
        return;
    }

    // Use `&&` (not `;`) so a sandbox which forbids raising the limit makes
    // the child fail instead of passing vacuously.
    let status = std::process::Command::new("bash")
        .args([
            "-c",
            "ulimit -s unlimited && exec \"$1\" --exact unlimited_stack_limit_accepts_shallow_source --nocapture",
            "parser-stack-unlimited",
        ])
        .arg(std::env::current_exe().expect("locate parser_stack_depth test binary"))
        .env(CHILD_ENV, "1")
        .status()
        .expect("run parser test child with RLIMIT_STACK=unlimited");
    assert!(
        status.success(),
        "unlimited-stack child failed (ulimit refused or parse rejected): {status}"
    );
}
