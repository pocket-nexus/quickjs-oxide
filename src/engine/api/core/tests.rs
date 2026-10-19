//! The corpus (`core/tests/corpus`) through `Context::eval` prints what the
//! lowering's driver prints.

use std::cell::RefCell;
use std::rc::Rc;

use super::{EvalOptions, QjsHelpers, Runtime, RuntimeError, RuntimeOptions, Value};
use crate::engine::compiler::CompileOptions;
use crate::engine::compiler::register::run_tests::{corpus, run_on_core};

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
    let mut context = runtime.context();
    context.install_qjs_helpers(&QjsHelpers::default()).unwrap();
    let mut result = context.eval(source).map(drop);
    if result.is_ok() {
        result = runtime.run_jobs();
    }
    let mut text = output.borrow().clone();
    if let Err(error) = result {
        assert_eq!(error, RuntimeError::Exception);
        let exception = context
            .take_exception()
            .unwrap()
            .unwrap_or(Value::Undefined);
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
    let mut context = runtime.context();
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
