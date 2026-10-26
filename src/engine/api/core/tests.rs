//! The corpus (`core/tests/corpus`) through `Context::eval` prints what the
//! lowering's driver prints.

use std::cell::RefCell;
use std::rc::Rc;

use super::{EvalOptions, QjsHelpers, Runtime, RuntimeError, RuntimeOptions, Value};
use crate::engine::api::core::compile::CompileOptions;
use crate::engine::mir::run_tests::{corpus, run_on_core};

#[derive(Default)]
struct Capture(Rc<RefCell<String>>);

impl oxide_core::host::Host for Capture {
    fn write(&mut self, _: oxide_core::host::Stream, text: &[u8]) {
        self.0.borrow_mut().push_str(&String::from_utf8_lossy(text));
    }
}

/// `source` through the API: `print` captured, the jobs drained, an
/// uncaught exception reported as `uncaught: <ToString>`.
fn run_api(source: &str) -> String {
    let output = Rc::default();
    let runtime = Runtime::with_host(Capture(Rc::clone(&output)), RuntimeOptions::default());
    let context = runtime.context();
    context.install_qjs_helpers(&QjsHelpers::default()).unwrap();
    let mut result = context.eval(source).map(drop);
    if result.is_ok() {
        result = runtime.run_jobs();
    }
    let mut text = output.borrow().clone();
    if let Err(error) = result {
        let RuntimeError::Exception(exception) = error else {
            panic!("not an exception: {error}");
        };
        let shown = context
            .to_js_string(&exception)
            .map_or_else(|_| "<unprintable>".to_owned(), |text| text.to_utf8_lossy());
        text.push_str(&format!("uncaught: {shown}\n"));
    }
    text
}

#[test]
fn the_corpus_prints_through_the_api_what_the_driver_prints() {
    let programs = corpus();
    assert_eq!(programs.len(), 46);
    let mut failures = Vec::new();
    for (name, source) in programs {
        let expected = run_on_core(&source);
        let actual = run_api(&source);
        if expected != actual {
            failures.push(format!(
                "==== {name}\n-- driver\n{expected}-- API\n{actual}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn transient_code_is_freed_once_nothing_names_it() {
    let runtime = Runtime::new();
    let context = runtime.context();
    runtime.run_gc().unwrap();
    let before = runtime.memory_report().unwrap().code;
    let transient = EvalOptions::default().transient();
    for i in 0..20 {
        let source = format!("(function () {{ return {{ k{i}: {i} }}; }})().k{i}");
        assert_eq!(
            context.eval_with_options(&source, &transient).unwrap(),
            Value::Int(i)
        );
    }
    let kept = context
        .compile_with_options(
            "(function () { return 'kept'; })()",
            &CompileOptions::default().transient(),
        )
        .unwrap();
    runtime.run_gc().unwrap();
    let after = runtime.memory_report().unwrap().code;
    assert_eq!(after.static_units, before.static_units);
    // Only the compiled script the embedder holds is left: its two units.
    assert_eq!(after.dynamic_units, before.dynamic_units + 2);
    let result = context.execute(&kept).unwrap();
    assert_eq!(
        context.to_js_string(&result).unwrap().to_utf8_lossy(),
        "kept"
    );
    drop(kept);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.memory_report().unwrap().code.dynamic_units,
        before.dynamic_units
    );
    // Code compiled without the option lives as long as the runtime.
    context.eval("(function () { return 1; })()").unwrap();
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.memory_report().unwrap().code.static_units,
        before.static_units + 2
    );
}

/// Defines a core host function as the global `name`, as the API's host
/// functions will be.
fn define_core_host(
    runtime: &Runtime,
    name: &str,
    constructor: bool,
    body: impl Fn(
        &mut oxide_core::NativeCx<'_>,
        oxide_core::vm::native::NativeCall,
    ) -> Result<oxide_core::Value, oxide_core::Throw>
    + 'static,
) {
    let mut inner = runtime.shared.inner.borrow_mut();
    let core = &mut inner.core;
    let body: Rc<oxide_core::vm::native::HostFn> = Rc::new(body);
    let function = core
        .native_cx()
        .new_host_function(name, 0, constructor, &[], body)
        .unwrap();
    let handle = core.hold(oxide_core::Value::object(function));
    let units: Vec<u16> = name.encode_utf16().collect();
    let key = core.key_from_units(&units).unwrap();
    let global = core.global_object();
    let value = oxide_core::Value::object(function);
    core.set(global, key, value, oxide_core::Value::object(global))
        .unwrap();
    core.release(handle);
}

#[test]
fn a_script_calls_a_host_function_and_catches_its_throw() {
    let runtime = Runtime::new();
    let context = runtime.context();
    define_core_host(&runtime, "twice", false, |cx, call| {
        let n = cx.to_number(call.arg(cx, 0))?;
        Ok(oxide_core::Value::number(n * 2.0))
    });
    define_core_host(&runtime, "fail", false, |cx, _| {
        let error = cx
            .new_error(oxide_core::ErrorKind::Type, "x is required")
            .unwrap();
        Err(oxide_core::Throw(error))
    });
    assert_eq!(context.eval("twice(21) === 42").unwrap(), Value::Bool(true));
    assert_eq!(
        context
            .eval("try { fail(); } catch (e) { e instanceof TypeError && e.message }")
            .unwrap(),
        Value::String(crate::engine::value::JsString::from_static("x is required"))
    );
    assert_eq!(
        context
            .eval("typeof twice + twice.name + twice.length")
            .unwrap(),
        Value::String(crate::engine::value::JsString::from_static(
            "functiontwice0"
        ))
    );
    assert!(matches!(
        context.eval("new twice()"),
        Err(RuntimeError::Exception(_))
    ));
}

#[test]
fn a_host_function_is_listed_in_a_backtrace() {
    let runtime = Runtime::new();
    let context = runtime.context();
    define_core_host(&runtime, "h", false, |cx, call| {
        cx.call(call.arg(cx, 0), oxide_core::Value::UNDEFINED, &[])
    });
    let stack = context
        .eval("try { h(function inner() { throw new Error('x'); }); } catch (e) { e.stack }")
        .unwrap();
    let Value::String(stack) = stack else {
        panic!("no stack");
    };
    let stack = stack.to_utf8_lossy();
    let lines: Vec<&str> = stack.lines().collect();
    assert!(lines[0].contains("at inner (<input>"), "{stack}");
    assert_eq!(lines[1].trim(), "at h (native)", "{stack}");
    assert!(lines[2].contains("at <eval> (<input>"), "{stack}");
}

#[test]
fn a_host_object_prints_as_an_object() {
    let runtime = Runtime::new();
    let context = runtime.context();
    define_core_host(&runtime, "make", false, |cx, _| {
        let prototype = cx.intrinsic(oxide_core::Intrinsic::ObjectPrototype);
        let object = cx.new_host_object(Some(prototype), Rc::new(7_u8)).unwrap();
        Ok(oxide_core::Value::object(object))
    });
    assert_eq!(
        context
            .eval("var o = make(); o.x = 1; typeof o + Object.prototype.toString.call(o) + o.x")
            .unwrap(),
        Value::String(crate::engine::value::JsString::from_static(
            "object[object Object]1"
        ))
    );
    let o = context.eval("o").unwrap();
    // The printer names the class `Object`, as it names every internal
    // record without a class of its own.
    assert_eq!(runtime.print_value(&o).unwrap(), "Object { x: 1 }");
}

#[test]
fn handles_a_host_function_drops_are_released_after_each_call() {
    use super::Scope;
    let runtime = Runtime::new();
    let context = runtime.context();
    let highest = Rc::new(std::cell::Cell::new(0usize));
    let seen = Rc::clone(&highest);
    let global = context.global_object().unwrap();
    context
        .define_function(&global, "touch", 1, move |scope: &mut Scope<'_>| {
            let argument = scope.arg(0);
            let live = scope.core.heap().handles_ref().live_count();
            seen.set(seen.get().max(live));
            drop(argument);
            Ok(Value::Undefined)
        })
        .unwrap();
    drop(global);
    context
        .eval("var o = {}; for (var i = 0; i < 10000000; i++) touch(o);")
        .unwrap();
    assert!(highest.get() < 16, "{} live handles", highest.get());
}
