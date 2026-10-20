use crate::object_graph_observation::object_property;
use crate::runtime_observation::exception_object;
use crate::runtime_observation::host_runtime_context;
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::core::{Context, ObjectRef, Runtime, Value};

// Pins QuickJS 2026-06-04's complete Proxy surface: all thirteen handler
// traps, null/undefined forwarding, revocation, callable/constructor caching,
// receiver/newTarget propagation, and the invariant checks in
// `js_proxy_*`. The `pinned quirks` group is deliberately not a statement of
// ideal ECMA-262 behavior. It records observable behavior in the selected
// QuickJS source, including its explicitly documented `proxy-missing-checks`.

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
    let error = exception_object(
        context.get_property(&proxy, &runtime.intern_property_key("answer").unwrap()),
        "revoked Proxy after collection",
    );
    assert_eq!(
        string_property(&runtime, &mut context, &error, "name"),
        "TypeError",
    );
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
