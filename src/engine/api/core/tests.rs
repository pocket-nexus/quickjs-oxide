//! Section 2.6: the two-core corpus through `Context::eval` prints what the
//! driver prints.

use std::cell::RefCell;
use std::rc::Rc;

use super::{QjsHelpers, Runtime, RuntimeError, RuntimeOptions, Value};
use crate::engine::compiler::register::run_tests::{corpus, run_new_core};

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
    assert_eq!(programs.len(), 44);
    let mut failures = Vec::new();
    for (name, source) in programs {
        let expected = run_new_core(&source);
        let actual = run_api(&source);
        if expected != actual {
            failures.push(format!(
                "==== {name}\n-- driver\n{expected}-- API\n{actual}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
