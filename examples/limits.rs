//! Errors and limits: exceptions, the call-stack and heap limits, the
//! collector and the memory report. Backs apps/web/docs/limits.md.
//!
//! ```sh
//! cargo run --quiet --example limits
//! ```

use quickjs_oxide::engine::api::core::{
    GcPolicy, HeapLimits, NativeErrorKind, Runtime, RuntimeError, RuntimeOptions, Value,
    declare_thread_stack,
};

fn text(value: &Value) -> String {
    match value {
        Value::String(text) => text.to_utf8_lossy(),
        other => panic!("expected a string, got {other:?}"),
    }
}

fn exceptions() {
    // docs:start exceptions
    let runtime = Runtime::new();
    let mut context = runtime.context();

    let result = context.eval("function f() { throw new TypeError('bad input') }\nf()");
    assert_eq!(result, Err(RuntimeError::Exception));
    assert!(context.has_exception().unwrap());

    // Taking the exception clears it.
    let Some(Value::Object(error)) = context.take_exception().unwrap() else {
        panic!("expected an error object");
    };
    assert!(!context.has_exception().unwrap());
    assert!(runtime.is_error_object(&error).unwrap());

    // print_value formats it as QuickJS's print does: the error, then its stack.
    let printed = runtime.print_value(&Value::Object(error)).unwrap();
    assert!(printed.starts_with("TypeError: bad input\n    at f (<input>:1"));
    // docs:end exceptions

    // A thrown non-error value comes back as is.
    assert_eq!(context.eval("throw 7"), Err(RuntimeError::Exception));
    assert_eq!(context.take_exception().unwrap(), Some(Value::Int(7)));
}

fn invariants() {
    // docs:start invariant
    let first = Runtime::new();
    let second = Runtime::new();
    let object = first.context().new_object().unwrap();
    assert!(matches!(
        second.context().set_property(
            &second.context().global_object().unwrap(),
            &second.intern_property_key("borrowed").unwrap(),
            Value::Object(object),
        ),
        Err(RuntimeError::Invariant(_))
    ));
    // docs:end invariant
}

fn syntax_errors() {
    // docs:start syntax-errors
    let runtime = Runtime::new();
    let mut context = runtime.context();

    // A syntax error is thrown, as QuickJS's JS_Eval throws it.
    assert_eq!(
        context.eval_with_filename("let x = ;", "app.js"),
        Err(RuntimeError::Exception)
    );
    let exception = context.take_exception().unwrap().unwrap();
    let printed = runtime.print_value(&exception).unwrap();
    assert!(printed.starts_with("SyntaxError: unexpected token in expression: ';'"));
    assert!(printed.contains("at app.js:1:9"));
    // docs:end syntax-errors
}

fn native_errors() {
    // docs:start native-error
    let runtime = Runtime::new();
    let context = runtime.context();
    let error = context
        .new_native_error(NativeErrorKind::Range, "out of range")
        .unwrap();
    let Value::Object(object) = &error else {
        panic!("expected an object")
    };
    assert!(runtime.is_error_object(object).unwrap());
    assert_eq!(NativeErrorKind::Range.name(), "RangeError");
    // docs:end native-error
}

fn stack_limits() {
    // docs:start stack-overflow
    let runtime = Runtime::new();
    let mut context = runtime.context();
    let caught = context
        .eval("function down(n) { return down(n + 1) + 1 }\ntry { down(0) } catch (e) { `${e.name}: ${e.message}` }")
        .unwrap();
    assert_eq!(text(&caught), "InternalError: stack overflow");
    // docs:end stack-overflow

    // docs:start stack-limit
    let depth = |options: RuntimeOptions| {
        let runtime = Runtime::with_options(options);
        let mut context = runtime.context();
        let reached = context
            .eval(
                "let depth = 0; function down() { depth++; down() }\ntry { down() } catch {} depth",
            )
            .unwrap();
        let Value::Int(reached) = reached else {
            panic!("expected a number")
        };
        reached
    };
    let small = depth(RuntimeOptions {
        stack_limit: 10_000,
        ..RuntimeOptions::default()
    });
    assert!(small < 10_000);
    assert!(depth(RuntimeOptions::default()) > small);
    // docs:end stack-limit

    // docs:start depth-limit
    // A host whose stack the byte measure cannot see bounds nesting by depth:
    // every native re-entry into the interpreter (here, each Array.prototype.map
    // callback) counts as one level.
    let runtime = Runtime::with_options(RuntimeOptions {
        native_depth_limit: 64,
        ..RuntimeOptions::default()
    });
    let mut context = runtime.context();
    let caught = context
        .eval(
            "function nest(n) { return n === 0 ? 0 : [n].map((m) => nest(m - 1))[0] + 1 }
             try { nest(1000) } catch (e) { `${e.name}: ${e.message}` }",
        )
        .unwrap();
    assert_eq!(text(&caught), "InternalError: stack overflow");
    assert_eq!(context.eval("nest(32)").unwrap(), Value::Int(32));
    // docs:end depth-limit
}

fn evaluation_thread() {
    // docs:start thread-stack
    const STACK: usize = 64 << 20;
    let worker = std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(|| {
            // First thing on the thread: the parser and the interpreter size
            // their stack checks from this.
            declare_thread_stack(STACK);
            let runtime = Runtime::new();
            let mut context = runtime.context();
            let nested = |depth: usize| format!("{}1{}", "[".repeat(depth), "]".repeat(depth));

            assert!(context.eval(&nested(500)).is_ok());

            // Nesting past the parser's budget is a catchable SyntaxError.
            assert_eq!(context.eval(&nested(5_000)), Err(RuntimeError::Exception));
            let exception = context.take_exception().unwrap().unwrap();
            let printed = runtime.print_value(&exception).unwrap();
            assert!(printed.starts_with("SyntaxError: stack overflow"));
        })
        .unwrap();
    worker.join().unwrap();
    // docs:end thread-stack
}

fn heap_cap() {
    // docs:start heap-cap
    let runtime = Runtime::with_options(RuntimeOptions::from(HeapLimits {
        max_bytes: 16 << 20,
    }));
    let mut context = runtime.context();

    // Reaching the cap throws a catchable InternalError.
    let caught = context
        .eval(
            "const chunks = [];
             try { for (;;) chunks.push(new Array(1024).fill(0)); }
             catch (e) { chunks.length = 0; `${e.name}: ${e.message}` }",
        )
        .unwrap();
    assert_eq!(text(&caught), "InternalError: out of memory");

    // The runtime stays usable once the memory is released.
    assert_eq!(context.eval("1 + 1").unwrap(), Value::Int(2));

    // Uncaught, the same error ends the call with RuntimeError::OutOfMemory.
    assert_eq!(
        context.eval("const kept = []; for (;;) kept.push(new Array(1024).fill(0));"),
        Err(RuntimeError::OutOfMemory)
    );
    assert!(!context.has_exception().unwrap());
    assert!(runtime.memory_report().unwrap().reserved_bytes() <= 16 << 20);
    // docs:end heap-cap
}

fn collector() {
    // docs:start collector
    let runtime = Runtime::with_options(RuntimeOptions {
        min_heap: 4 << 20, // collect no earlier than at 4 MiB reserved
        growth: 1.5,       // then at (1 + 1.5) x the live bytes
        ..RuntimeOptions::default()
    });
    runtime.set_gc_policy(GcPolicy::Manual).unwrap();
    let mut context = runtime.context();

    context
        .eval("let garbage = []; for (let i = 0; i < 10000; i++) garbage.push({ i }); garbage = null;")
        .unwrap();
    let before = runtime.gc_stats().unwrap().collections;
    let stats = runtime.run_gc().unwrap();
    assert_eq!(stats.collections, before + 1);
    assert!(stats.freed_records >= 10_000);

    let report = runtime.memory_report().unwrap();
    assert!(report.objects.live > 0);
    assert_eq!(report.cap, usize::MAX);
    // Display formats it as `qjs --memory-report` prints it.
    assert!(report.to_string().contains("heap reserved"));
    // docs:end collector
}

fn main() {
    exceptions();
    syntax_errors();
    invariants();
    native_errors();
    stack_limits();
    evaluation_thread();
    heap_cap();
    collector();
    println!("limits: ok");
}
