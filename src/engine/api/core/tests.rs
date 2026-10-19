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

#[test]
fn host_functions_are_called_throw_and_call_back() {
    use super::{NativeErrorKind, PropertyKey};
    let output = Rc::default();
    let runtime = Runtime::with_host(Capture(Rc::clone(&output)), RuntimeOptions::default());
    let mut context = runtime.context();
    context.install_qjs_helpers(&QjsHelpers::default()).unwrap();
    let add = context
        .new_function("add", 2, |scope| {
            let a = scope.arg(0);
            let b = scope.arg(1);
            let sum = scope.to_number(&a)? + scope.to_number(&b)?;
            Ok(Value::number(sum))
        })
        .unwrap();
    let fail = context
        .new_function("fail", 1, |scope| {
            let message = scope.arg(0);
            let message = scope.to_string(&message)?;
            Err(scope.error(NativeErrorKind::Range, &message))
        })
        .unwrap();
    let apply = context
        .new_function("apply", 2, |scope| {
            let function = scope.arg(0);
            let argument = scope.arg(1);
            let point = scope.new_object()?;
            scope.define(&point, "x", &argument)?;
            let nested = scope.new_function("double", 1, |scope| {
                let value = scope.arg(0);
                Ok(Value::number(scope.to_number(&value)? * 2.0))
            })?;
            scope.define(&point, "double", &Value::Object(nested.into_object()))?;
            scope.call(&function, &Value::Undefined, &[Value::Object(point)])
        })
        .unwrap();
    let make = context
        .new_constructor("Point", 1, |scope| {
            let constructed = scope.new_target().is_some();
            let object = scope.new_object()?;
            let x = scope.arg(0);
            scope.define(&object, "x", &x)?;
            scope.define(&object, "constructed", &Value::Bool(constructed))?;
            Ok(Value::Object(object))
        })
        .unwrap();
    let global = context.global_object().unwrap();
    for (name, function) in [
        ("add", add),
        ("fail", fail),
        ("apply", apply),
        ("Point", make),
    ] {
        let key = PropertyKey::from_js_string(
            crate::engine::value::JsString::try_from_utf8(name).unwrap(),
        );
        context
            .set_property(&global, &key, Value::Object(function.into_object()))
            .unwrap();
    }
    context
        .eval(
            r#"
            print(add(2, 3), add.name, add.length);
            try { fail("boom"); } catch (e) { print(e instanceof RangeError, e.message); }
            print(apply(p => p.double(p.x) + 1, 20));
            const p = new Point(7);
            print(p.x, p.constructed, Point(1).constructed);
            try { add({ valueOf() { throw new Error("inner"); } }, 1); } catch (e) { print(e.message); }
            "#,
        )
        .unwrap();
    assert_eq!(
        output.borrow().as_str(),
        "5 add 2\ntrue boom\n41\n7 true false\ninner\n"
    );
}

#[test]
fn host_functions_read_and_write_buffer_bytes() {
    use super::PropertyKey;
    let output = Rc::default();
    let runtime = Runtime::with_host(Capture(Rc::clone(&output)), RuntimeOptions::default());
    let mut context = runtime.context();
    context.install_qjs_helpers(&QjsHelpers::default()).unwrap();
    let sum = context
        .new_function("sum", 1, |scope| {
            let view = scope.arg(0);
            Ok(match scope.bytes(&view) {
                Some(bytes) => Value::number(bytes.iter().map(|&b| f64::from(b)).sum()),
                None => Value::number(-1.0),
            })
        })
        .unwrap();
    let fill = context
        .new_function("fill", 2, |scope| {
            let view = scope.arg(0);
            let start = scope.arg(1);
            let start = scope.to_number(&start)? as u8;
            match scope.bytes_mut(&view) {
                Some(bytes) => {
                    for (index, byte) in bytes.iter_mut().enumerate() {
                        *byte = start.wrapping_add(index as u8);
                    }
                    Ok(Value::number(bytes.len() as f64))
                }
                None => Err(scope.type_error("not a buffer")),
            }
        })
        .unwrap();
    let make = context
        .new_function("make", 1, |scope| {
            let len = scope.arg(0);
            let len = scope.to_number(&len)? as u8;
            let buffer = scope.new_array_buffer((0..len).collect())?;
            Ok(Value::Object(buffer))
        })
        .unwrap();
    let global = context.global_object().unwrap();
    for (name, function) in [("sum", sum), ("fill", fill), ("make", make)] {
        let key = PropertyKey::from_js_string(
            crate::engine::value::JsString::try_from_utf8(name).unwrap(),
        );
        context
            .set_property(&global, &key, Value::Object(function.into_object()))
            .unwrap();
    }
    context
        .eval(
            r#"
            const buf = new ArrayBuffer(8);
            print(fill(new Uint8Array(buf, 2, 4), 10), new Uint8Array(buf).join());
            print(sum(buf), sum(new Uint16Array(buf, 2, 2)), sum(new DataView(buf, 3, 2)));
            const made = make(3);
            print(made instanceof ArrayBuffer, made.byteLength, new Uint8Array(made).join());
            print(sum({}), sum(1), sum(new SharedArrayBuffer(4)));
            const gone = new ArrayBuffer(4);
            const view = new Uint8Array(gone);
            gone.transfer();
            print(sum(gone), sum(view));
            const resizable = new ArrayBuffer(4, { maxByteLength: 8 });
            const tail = new Uint8Array(resizable, 2, 2);
            fill(resizable, 5);
            resizable.resize(3);
            print(sum(tail), sum(new Uint8Array(resizable)));
            try { fill({}, 0); } catch (e) { print(e instanceof TypeError, e.message); }
            "#,
        )
        .unwrap();
    assert_eq!(
        output.borrow().as_str(),
        "4 0,0,10,11,12,13,0,0\n46 46 23\ntrue 3 0,1,2\n-1 -1 -1\n-1 -1\n-1 18\ntrue not a buffer\n"
    );
}
