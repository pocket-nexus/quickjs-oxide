use crate::runtime_observation::host_runtime;
use quickjs_oxide::engine::api::core::{CallableRef, Context, Runtime, Value};

#[test]
fn global_this_var_ref_cycle_is_collectable_after_context_drop() {
    let runtime = host_runtime();
    {
        let mut context = runtime.new_context().expect("create context");
        let global = context.global_object().unwrap();
        let reader = function(
            &runtime,
            &mut context,
            "(function() { return globalThis; })",
        );
        assert_eq!(
            context.call(&reader, Value::Undefined, &[]).unwrap(),
            Value::Object(global)
        );
    }

    runtime.run_gc().unwrap();
}

fn function(runtime: &Runtime, context: &mut Context, source: &str) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("function source did not produce an object: {source}");
    };
    runtime.as_callable(&object).unwrap().unwrap()
}
