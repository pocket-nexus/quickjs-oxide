//! The walk-through of the "Embed quickjs-oxide" docs page; every value the
//! page shows is asserted here.
//!
//! ```sh
//! cargo run --quiet --example embed
//! ```

// docs:start imports
use quickjs_oxide::engine::api::core::{JsString, QjsHelpers, Runtime, RuntimeError, Value};
use quickjs_oxide_host::SystemHost;
// docs:end imports

fn main() -> Result<(), Box<dyn std::error::Error>> {
    first_eval()?;
    read_result()?;
    call_function()?;
    catch_exception()?;
    console()?;
    Ok(())
}

fn first_eval() -> Result<(), Box<dyn std::error::Error>> {
    // docs:start first-eval
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context()?;

    let value = context.eval("[1, 2, 3].map((x) => x * 2).join()")?;

    let Value::String(text) = value else {
        panic!("expected a string, got {value:?}");
    };
    assert_eq!(text.to_utf8_lossy(), "2,4,6");
    // docs:end first-eval
    Ok(())
}

fn read_result() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context()?;
    // docs:start read-result
    assert_eq!(context.eval("6 * 7")?, Value::Int(42));
    assert_eq!(context.eval("0.5 + 0.25")?, Value::Float(0.75));
    assert_eq!(context.eval("1 < 2")?, Value::Bool(true));
    assert_eq!(context.eval("var unused = 1")?, Value::Undefined);

    // Numbers: `as_number` reads both `Int` and `Float`.
    let total = context.eval("[1.5, 2, 3].reduce((a, b) => a + b)")?;
    assert_eq!(total.as_number(), Some(6.5));

    // Objects come back as handles; read their properties through the context.
    let point = context.eval("({ x: 3, y: 4 })")?;
    let point = point.as_object().expect("an object");
    let x = runtime.intern_property_key("x")?;
    assert_eq!(context.get_property(point, &x)?, Value::Int(3));
    // docs:end read-result
    Ok(())
}

fn call_function() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context()?;
    // docs:start call-function
    // Define a function in JavaScript.
    context.eval(
        "function greet(name, times) { return ('hi ' + name + ' ').repeat(times).trim(); }",
    )?;

    // Look it up on the global object and check that it is callable.
    let global = context.global_object()?;
    let key = runtime.intern_property_key("greet")?;
    let Value::Object(object) = context.get_property(&global, &key)? else {
        panic!("greet is not an object");
    };
    let greet = runtime.as_callable(&object)?.expect("greet is callable");

    // Call it with `this` undefined and two arguments.
    let name = Value::String(JsString::try_from_utf8("Ada")?);
    let result = context.call(&greet, Value::Undefined, &[name, Value::Int(2)])?;
    let text = context.to_js_string(&result)?;
    assert_eq!(text.to_utf8_lossy(), "hi Ada hi Ada");
    // docs:end call-function
    Ok(())
}

fn catch_exception() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context()?;
    // docs:start exception
    match context.eval("null.length") {
        Err(RuntimeError::Exception) => {
            // The thrown value waits on the context until it is taken.
            let thrown = context.take_exception()?.expect("a pending exception");
            let message = context.to_js_string(&thrown)?;
            assert_eq!(
                message.to_utf8_lossy(),
                "TypeError: cannot read property 'length' of null"
            );
        }
        other => panic!("expected an exception, got {other:?}"),
    }
    // docs:end exception
    Ok(())
}

fn console() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.new_context()?;
    // docs:start console
    // A fresh realm has the standard built-ins only.
    assert_eq!(
        context.eval("typeof console")?,
        Value::String(JsString::from_static("undefined"))
    );

    // Add qjs's print and console.log; they write through the host (stdout here).
    context.install_qjs_helpers(&QjsHelpers::default())?;
    context.eval("console.log('hello from JavaScript')")?;
    // docs:end console
    Ok(())
}
