use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable_with_read_context as property_callable;
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::core::{CallableRef, Context, Runtime, Value};

// Differential lock for the complete observable Date slice in pinned QuickJS
// 2026-06-04. Every JavaScript vector stays inside quickjs-oxide's currently
// implemented grammar; in particular it avoids classes, arrow functions,
// destructuring, regular expressions, and async/module syntax.

#[test]
fn detached_date_and_native_method_keep_their_defining_realm_collectable() {
    let runtime = host_runtime();
    let (date, value_of) = {
        let mut defining = runtime.new_context().expect("create context");
        let date = eval_object(&mut defining, "new Date(42)", "detached Date");
        let value_of = date_prototype_callable(&runtime, &mut defining, "valueOf");
        (date, value_of)
    };

    runtime.run_gc().unwrap();

    {
        let caller = runtime.new_context().expect("create context");
        assert_eq!(
            caller
                .call(
                    &value_of,
                    Value::Object(date.try_clone().expect("duplicate root")),
                    &[]
                )
                .unwrap()
                .as_number(),
            Some(42.0),
            "detached Date and valueOf stopped working after their Context dropped",
        );
    }

    drop(value_of);
    runtime.run_gc().unwrap();

    drop(date);
    runtime.run_gc().unwrap();
}

fn date_prototype_callable(runtime: &Runtime, context: &mut Context, name: &str) -> CallableRef {
    let prototype = eval_object(context, "Date.prototype", "Date prototype");
    property_callable(runtime, context, &prototype, name)
}
