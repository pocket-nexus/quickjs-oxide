//! Rust and JavaScript calling each other: host functions, a host object,
//! a promise settled after the script returns, a native module and an
//! interrupt. Backs apps/web/docs/functions.md.
//!
//! ```sh
//! cargo run --quiet --example host
//! ```

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use quickjs_oxide::engine::api::core::{
    FunctionOptions, JsString, NativeErrorKind, PromiseCapability, Runtime, RuntimeError, Scope,
    Value,
};

/// A host object's value: shared, so mutation goes through a `Cell`.
struct Counter {
    count: Cell<i32>,
}

fn main() -> Result<(), RuntimeError> {
    let runtime = Runtime::new();
    let context = runtime.context();
    // `print`, which the scripts below use.
    context.install_qjs_helpers(&Default::default())?;
    let global = context.global_object()?;

    // docs:start typed-function
    // A function on the global object, its arguments converted.
    context.define_function(&global, "add", 2, |_: &mut Scope<'_>, a: f64, b: f64| {
        Ok(a + b)
    })?;
    // docs:end typed-function

    // docs:start data
    // A function whose captured value lives in the heap (`data`), where the
    // collector sees it.
    let greet = context.new_function(
        FunctionOptions::new("greet")
            .length(1)
            .data(vec![context.eval("'Hello'")?]),
        |scope: &mut Scope<'_>| {
            let greeting = scope.data(0);
            let greeting = scope.to_js_string(&greeting)?.to_utf8_lossy();
            let name = scope.arg(0);
            let name = scope.to_js_string(&name)?.to_utf8_lossy();
            Ok(Value::String(JsString::try_from_utf8(&format!(
                "{greeting}, {name}!"
            ))?))
        },
    )?;
    let key = runtime.intern_property_key("greet")?;
    context.set_property(&global, &key, Value::from(greet))?;
    // docs:end data

    // docs:start host-object
    // A host object whose method reads its value.
    let prototype = context.new_object()?;
    context.define_function(&prototype, "increment", 0, |scope: &mut Scope<'_>| {
        let Some(counter) = scope.this_host_data::<Counter>()? else {
            return Err(scope.throw_error(NativeErrorKind::Type, "not a Counter"));
        };
        counter.count.set(counter.count.get() + 1);
        Ok(Value::Int(counter.count.get()))
    })?;
    let counter = context.new_host_object(
        Counter {
            count: Cell::new(0),
        },
        Some(&prototype),
    )?;
    let key = runtime.intern_property_key("counter")?;
    context.set_property(&global, &key, Value::Object(counter))?;
    // docs:end host-object

    // docs:start call-back
    // A function that calls back into a script closure, and one that throws
    // a TypeError the script catches.
    context.define_function(&global, "twice", 1, |scope: &mut Scope<'_>| {
        let Value::Object(function) = scope.arg(0) else {
            return Err(scope.throw_error(NativeErrorKind::Type, "twice needs a function"));
        };
        let Some(function) = scope.as_callable(&function)? else {
            return Err(scope.throw_error(NativeErrorKind::Type, "twice needs a function"));
        };
        let once = scope.call(&function, Value::Undefined, &[Value::Int(1)])?;
        scope.call(&function, Value::Undefined, &[once])
    })?;
    // docs:end call-back

    let shown = context.eval(
        "var lines = [];\n\
         lines.push(add(40, 2));\n\
         lines.push(greet('embedder'));\n\
         counter.increment(); lines.push(counter.increment());\n\
         lines.push(twice((x) => x * 10));\n\
         try { twice(5); } catch (e) { lines.push(e.name + ': ' + e.message); }\n\
         lines.join('\\n')",
    )?;
    println!("{}", context.to_js_string(&shown)?.to_utf8_lossy());

    // docs:start promise
    // A promise the script awaits, resolved after the script returned: the
    // host function keeps the capability, the embedder settles it and
    // drains the job queue.
    let pending: Rc<RefCell<Option<PromiseCapability>>> = Rc::default();
    let keep = Rc::clone(&pending);
    context.define_function(&global, "fetchLater", 0, move |scope: &mut Scope<'_>| {
        let capability = scope.new_promise()?;
        let promise = capability.promise.clone();
        *keep.borrow_mut() = Some(capability);
        Ok(Value::Object(promise))
    })?;
    context.eval("(async () => { print('fetched: ' + await fetchLater()); })()")?;
    let capability = pending.borrow_mut().take().expect("the script asked");
    let mut context = context;
    let data = context.eval("'the data'")?;
    capability.resolve(&mut context, data)?;
    runtime.run_jobs()?;
    // docs:end promise

    // docs:start native-module
    // A native module, imported by a module script.
    let sqrt = context.new_function(
        FunctionOptions::new("sqrt").length(1),
        |_: &mut Scope<'_>, x: f64| Ok(x.sqrt()),
    )?;
    context.define_module("host:math", vec![("sqrt".into(), Value::from(sqrt))])?;
    let module = context.compile_module(
        "import { sqrt } from 'host:math';\n\
         print('sqrt(2) = ' + sqrt(2).toFixed(4));",
    )?;
    context.execute_module(&module)?;
    runtime.run_jobs()?;
    // docs:end native-module

    // docs:start interrupt
    // An interrupt from another thread stops an endless loop; no `catch`
    // runs.
    let handle = runtime.interrupt_handle();
    let thread = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(20));
        handle.interrupt();
    });
    let outcome = context.eval("try { while (true) {} } catch (e) { 'caught' }");
    thread.join().expect("the interrupting thread");
    println!("endless loop: {outcome:?}");
    runtime.interrupt_handle().clear();
    // docs:end interrupt
    Ok(())
}
