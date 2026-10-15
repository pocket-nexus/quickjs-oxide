use crate::object_graph_observation::require_object as expect_object;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{CallableRef, Context, Runtime, RuntimeError, Value};

// Differential lock for the complete observable Date slice in pinned QuickJS
// 2026-06-04. Every JavaScript vector stays inside quickjs-oxide's currently
// implemented grammar; in particular it avoids classes, arrow functions,
// destructuring, regular expressions, and async/module syntax.

#[test]
fn date_cross_realm_prototypes_fallback_and_errors_use_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let defining_constructor = date_constructor(&runtime, &mut defining);
    let defining_date_prototype =
        eval_object(&mut defining, "Date.prototype", "defining Date prototype");
    let caller_date_prototype = eval_object(&mut caller, "Date.prototype", "caller Date prototype");
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    assert_ne!(defining_date_prototype, caller_date_prototype);
    assert_ne!(defining_type_error, caller_type_error);

    let foreign_date = expect_object(
        caller
            .construct(&defining_constructor, &[Value::Int(42)])
            .unwrap(),
        "foreign Date construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&foreign_date).unwrap(),
        Some(defining_date_prototype.try_clone().expect("duplicate root")),
        "Date construction did not use the foreign constructor prototype",
    );

    let caller_new_target = eval_callable(
        &runtime,
        &mut caller,
        "(function(){function F(){}F.prototype=17;return F})()",
        "caller primitive-prototype newTarget",
    );
    let fallback_date = expect_object(
        caller
            .construct_with_new_target(&defining_constructor, &caller_new_target, &[Value::Int(7)])
            .unwrap(),
        "cross-realm Date fallback construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&fallback_date).unwrap(),
        Some(caller_date_prototype),
        "primitive newTarget.prototype did not fall back to the newTarget realm",
    );

    let value_of = date_prototype_callable(&runtime, &mut defining, "valueOf");
    let caller_date = eval_object(&mut caller, "new Date(123)", "caller Date");
    assert_eq!(
        caller
            .call(&value_of, Value::Object(caller_date), &[])
            .unwrap()
            .as_number(),
        Some(123.0),
        "foreign Date valueOf rejected a genuine same-runtime Date",
    );

    let ordinary = eval_object(&mut caller, "({})", "caller ordinary object");
    assert_eq!(
        caller.call(&value_of, Value::Object(ordinary), &[]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "foreign Date valueOf TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Date branded native error used the calling realm",
    );

    let to_json = date_prototype_callable(&runtime, &mut defining, "toJSON");
    let throwing = eval_object(
        &mut caller,
        r#"(function(){
            var value=Object();
            value.valueOf=function(){throw new TypeError("caller conversion")};
            return value;
        })()"#,
        "caller throwing toJSON receiver",
    );
    assert_eq!(
        caller.call(&to_json, Value::Object(throwing), &[]),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller toJSON conversion TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Date.toJSON replaced a caller-realm user error",
    );
}

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
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1,
        "detached Date roots must retain their defining realm graph",
    );

    {
        let mut caller = runtime.new_context().expect("create context");
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
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1,
        "the detached Date must retain its prototype realm after valueOf drops",
    );

    drop(date);
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").live,
        0,
        "Date realms, prototypes, native functions, and instances must be collectable",
    );
}

fn date_constructor(runtime: &Runtime, context: &mut Context) -> CallableRef {
    let global = context.global_object().unwrap();
    property_callable(runtime, context, &global, "Date")
}

fn date_prototype_callable(runtime: &Runtime, context: &mut Context, name: &str) -> CallableRef {
    let prototype = eval_object(context, "Date.prototype", "Date prototype");
    property_callable(runtime, context, &prototype, name)
}
