use crate::object_graph_observation::intrinsic_prototype;
use crate::runtime_observation::error_string;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{property_callable, take_error_object as take_exception_object};
use quickjs_oxide::engine::api::{
    Context, DescriptorField, JsString, ObjectRef, OrdinaryPropertyDescriptor, Runtime,
    RuntimeError, Value,
};

#[test]
fn string_rope_overflow_uses_vm_and_native_defining_realms() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_string = first.string_prototype().unwrap();
    let concat = property_callable(&runtime, &mut first, &first_string, "concat");
    let first_internal_error = intrinsic_prototype(&runtime, &mut first, "InternalError");
    let second_internal_error = intrinsic_prototype(&runtime, &mut second, "InternalError");
    assert_ne!(first_internal_error, second_internal_error);

    let near = near_limit_rope();
    let second_global = second.global_object().unwrap();
    define_data(
        &runtime,
        &second_global,
        "nearLimitString",
        Value::String(near.clone()),
    );
    assert_eq!(
        second.eval("nearLimitString + nearLimitString"),
        Err(RuntimeError::Exception)
    );
    let vm_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&vm_error).unwrap(),
        Some(second_internal_error)
    );
    assert_internal_string_too_long(&runtime, &mut second, &vm_error);

    assert_eq!(
        second.call(
            &concat,
            Value::String(JsString::try_from_utf8("").unwrap()),
            &[Value::String(near.clone()), Value::String(near)],
        ),
        Err(RuntimeError::Exception)
    );
    let native_error = take_exception_object(&mut second);
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(first_internal_error)
    );
    assert_internal_string_too_long(&runtime, &mut second, &native_error);
}

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
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    assert_eq!(rope.len(), 536_936_448);
    assert_eq!(rope.code_unit_at(0), Some(u16::from(b'x')));
    assert_eq!(rope.code_unit_at(rope.len() - 1), Some(u16::from(b'x')));

    drop(method);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
    drop(rope);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
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

fn assert_internal_string_too_long(runtime: &Runtime, context: &mut Context, error: &ObjectRef) {
    assert_eq!(
        error_string(runtime, context, error, "name"),
        JsString::try_from_utf8("InternalError").unwrap()
    );
    assert_eq!(
        error_string(runtime, context, error, "message"),
        JsString::try_from_utf8("string too long").unwrap()
    );
}
