use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::core::{
    DescriptorField, JsString, ObjectRef, OrdinaryPropertyDescriptor, Runtime, Value,
};

#[test]
fn string_rope_dag_does_not_keep_its_realm_graph_alive() {
    let runtime = host_runtime();
    let rope = near_limit_rope();
    let method = {
        let mut context = runtime.new_context().expect("create context");
        let global = context.global_object().unwrap();
        define_data(&runtime, &global, "rootedRope", Value::String(rope.clone()));
        let prototype = context.string_prototype().unwrap();
        property_callable(&runtime, &mut context, &prototype, "concat")
    };

    runtime.run_gc().unwrap();
    assert_eq!(rope.len(), 536_936_448);
    assert_eq!(rope.code_unit_at(0), Some(u16::from(b'x')));
    assert_eq!(rope.code_unit_at(rope.len() - 1), Some(u16::from(b'x')));

    drop(method);
    runtime.run_gc().unwrap();
    drop(rope);
    runtime.run_gc().unwrap();
}

fn near_limit_rope() -> JsString {
    let mut value = JsString::try_from_utf8(&"x".repeat(8193)).unwrap();
    for _ in 0..16 {
        value = value.try_concat(&value).unwrap();
    }
    assert_eq!(value.len(), 536_936_448);
    value
}

fn define_data(runtime: &Runtime, object: &ObjectRef, name: &str, value: Value) {
    let key = runtime.intern_property_key(name).unwrap();
    assert!(
        runtime
            .define_own_property(
                object,
                &key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(value),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}
