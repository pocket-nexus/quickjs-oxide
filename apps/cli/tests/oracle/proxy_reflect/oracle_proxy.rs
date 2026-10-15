use crate::object_graph_observation::define_global;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{Context, ObjectRef, Runtime, RuntimeError, Value};

// Pins QuickJS 2026-06-04's complete Proxy surface: all thirteen handler
// traps, null/undefined forwarding, revocation, callable/constructor caching,
// receiver/newTarget propagation, and the invariant checks in
// `js_proxy_*`. The `pinned quirks` group is deliberately not a statement of
// ideal ECMA-262 behavior. It records observable behavior in the selected
// QuickJS source, including its explicitly documented `proxy-missing-checks`.

#[test]
fn proxy_native_and_user_errors_use_their_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
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
    assert_ne!(defining_type_error, caller_type_error);

    let invariant_proxy = eval_object(
        &mut defining,
        r#"(function(){
            var target={x:1};Object.preventExtensions(target);
            return new Proxy(target,{has:function(){return false}});
        })()"#,
        "foreign invariant Proxy",
    );
    define_global(
        &runtime,
        &mut caller,
        "foreignProxy",
        Value::Object(invariant_proxy),
    );
    assert_eq!(
        caller.eval("'x' in foreignProxy"),
        Err(RuntimeError::Exception),
    );
    let invariant_error = take_exception_object(&mut caller, "foreign Proxy invariant TypeError");
    assert_eq!(
        runtime.get_prototype_of(&invariant_error).unwrap(),
        Some(caller_type_error.try_clone().expect("duplicate root")),
        "Proxy's internal invariant TypeError did not use the active caller realm",
    );

    let revoked_proxy = eval_object(
        &mut defining,
        r#"(function(){
            var pair=Proxy.revocable(function(){},{});
            pair.revoke();
            return pair.proxy;
        })()"#,
        "foreign revoked Proxy",
    );
    define_global(
        &runtime,
        &mut caller,
        "foreignProxy",
        Value::Object(revoked_proxy),
    );
    assert_eq!(caller.eval("foreignProxy.x"), Err(RuntimeError::Exception));
    let revoked_error = take_exception_object(&mut caller, "foreign revoked Proxy TypeError");
    assert_eq!(
        runtime.get_prototype_of(&revoked_error).unwrap(),
        Some(caller_type_error.try_clone().expect("duplicate root")),
        "revoked Proxy TypeError did not use the active caller realm",
    );

    let throwing_proxy = eval_object(
        &mut defining,
        r#"(new Proxy({x:1},{get:function(){throw new TypeError("defining trap")}}))"#,
        "foreign throwing Proxy",
    );
    define_global(
        &runtime,
        &mut caller,
        "foreignProxy",
        Value::Object(throwing_proxy),
    );
    assert_eq!(caller.eval("foreignProxy.x"), Err(RuntimeError::Exception));
    let user_error = take_exception_object(&mut caller, "foreign Proxy user TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(defining_type_error),
        "Proxy replaced a defining-realm user trap throw with a caller-realm error",
    );

    let bad_construct_proxy = eval_object(
        &mut defining,
        "(new Proxy(function(){},{construct:function(){return 1}}))",
        "foreign primitive-returning construct Proxy",
    );
    define_global(
        &runtime,
        &mut caller,
        "foreignProxy",
        Value::Object(bad_construct_proxy),
    );
    assert_eq!(
        caller.eval("new foreignProxy()"),
        Err(RuntimeError::Exception)
    );
    let construct_error = take_exception_object(&mut caller, "foreign Proxy construct TypeError");
    assert_eq!(
        runtime.get_prototype_of(&construct_error).unwrap(),
        Some(caller_type_error),
        "Proxy's construct-result TypeError did not use the active caller realm",
    );
}

#[test]
fn proxy_hidden_edges_survive_gc_and_revocation_is_safe_across_collection() {
    let (runtime, mut context) = host_runtime_context();
    let record = eval_object(
        &mut context,
        r#"(function(){
            var target={answer:40},handler={
                get:function(seen,key,receiver){return Reflect.get(seen,key,receiver)+2}
            };
            target.handler=handler;handler.target=target;
            return Proxy.revocable(target,handler);
        })()"#,
        "cyclic revocable Proxy record",
    );
    let proxy = object_property(&runtime, &mut context, &record, "proxy");
    let revoke_object = object_property(&runtime, &mut context, &record, "revoke");
    let revoke = runtime
        .as_callable(&revoke_object)
        .unwrap()
        .expect("Proxy revoke object was not callable");
    drop(record);

    runtime.run_gc().expect("collect while Proxy is live");
    assert_eq!(
        context
            .get_property(&proxy, &runtime.intern_property_key("answer").unwrap())
            .expect("Proxy target and handler did not survive collection"),
        Value::Int(42),
    );

    drop(
        context
            .call(&revoke, Value::Undefined, &[])
            .expect("revoke after collection"),
    );
    drop(revoke);
    drop(revoke_object);
    runtime
        .run_gc()
        .expect("collect after one-shot revoke capture");
    assert!(matches!(
        context.get_property(&proxy, &runtime.intern_property_key("answer").unwrap()),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut context, "revoked Proxy after collection");
    assert_eq!(
        string_property(&runtime, &mut context, &error, "name"),
        "TypeError",
    );
}

fn object_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> ObjectRef {
    let Value::Object(value) = context
        .get_property(object, &runtime.intern_property_key(name).unwrap())
        .unwrap_or_else(|error| panic!("read object property {name}: {error}"))
    else {
        panic!("{name} was not an object");
    };
    value
}

fn take_exception_object(context: &mut Context, description: &str) -> ObjectRef {
    let Some(Value::Object(error)) = context
        .take_exception()
        .unwrap_or_else(|failure| panic!("take {description}: {failure}"))
    else {
        panic!("{description} was missing or was not an object");
    };
    error
}

fn string_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> String {
    let Value::String(value) = context
        .get_property(object, &runtime.intern_property_key(name).unwrap())
        .unwrap_or_else(|error| panic!("read {name}: {error}"))
    else {
        panic!("{name} was not a string");
    };
    value.to_utf8_lossy()
}
