use crate::runtime_observation::host_runtime_context;
use quickjs_oxide::engine::api::core::Value;

#[test]
fn context_detach_array_buffer_uses_the_same_idempotent_core() {
    let (_runtime, context) = host_runtime_context();
    let buffer = context
        .eval("globalThis.__buffer=new ArrayBuffer(4,{maxByteLength:8});__buffer")
        .unwrap();
    context.detach_array_buffer(&Value::Undefined).unwrap();
    let ordinary = Value::Object(context.new_object().unwrap());
    context.detach_array_buffer(&ordinary).unwrap();
    context.detach_array_buffer(&buffer).unwrap();
    context.detach_array_buffer(&buffer).unwrap();
    let Value::String(observation) = context
        .eval(
            "[__buffer.byteLength,__buffer.maxByteLength,\
             __buffer.resizable,__buffer.detached].join('|')",
        )
        .unwrap()
    else {
        panic!("detach observation was not a string");
    };
    // 25.1.6.4: a detached buffer's maxByteLength is 0.
    assert_eq!(observation.to_utf8_lossy(), "0|0|true|true");
}
