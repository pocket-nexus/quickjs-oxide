//! The examples of the "API and commands" docs page; every value the page
//! shows is asserted here.
//!
//! ```sh
//! cargo run --quiet --example api
//! ```

use quickjs_oxide::engine::api::core::{
    EvalOptions, HeapLimits, Runtime, RuntimeError, RuntimeOptions, Value, declare_thread_stack,
};
use quickjs_oxide_host::SystemHost;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    options()?;
    eval_options()?;
    thread_stack()?;
    Ok(())
}

fn options() -> Result<(), Box<dyn std::error::Error>> {
    // docs:start runtime-options
    let options = RuntimeOptions {
        heap: HeapLimits {
            max_bytes: 16 << 20, // 16 MiB
        },
        ..RuntimeOptions::default()
    };
    let runtime = Runtime::with_host(SystemHost::default(), options);
    let mut context = runtime.context();

    // Past the cap, an allocation fails with OutOfMemory.
    let result = context.eval("const a = []; for (;;) a.push({})");
    assert!(matches!(result, Err(RuntimeError::OutOfMemory)));
    // docs:end runtime-options
    Ok(())
}

fn eval_options() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = Runtime::new_with_host_services(SystemHost::default());
    let mut context = runtime.context();
    // docs:start eval-options
    let options = EvalOptions::new("line-1.js").transient();
    assert_eq!(context.eval_with_options("1 + 1", &options)?, Value::Int(2));
    // docs:end eval-options
    Ok(())
}

fn thread_stack() -> Result<(), Box<dyn std::error::Error>> {
    // docs:start thread-stack
    const STACK: usize = 32 << 20; // 32 MiB
    let worker = std::thread::Builder::new().stack_size(STACK).spawn(|| {
        declare_thread_stack(STACK);
        let runtime = Runtime::new_with_host_services(SystemHost::default());
        let mut context = runtime.context();
        // Each level re-enters the interpreter through Array.prototype.map.
        let source =
            "function depth(n) { return n === 0 ? 0 : 1 + [n - 1].map(depth)[0]; } depth(1000)";
        // A Value stays on its runtime's thread; send back plain data.
        context.eval(source).map(|value| value.as_number())
    })?;
    let result = worker.join().expect("the worker finished");
    assert_eq!(result, Ok(Some(1000.0)));
    // docs:end thread-stack
    Ok(())
}
