use quickjs_oxide::engine::api::core::{Runtime, RuntimeError, Value};

#[allow(dead_code)]
pub fn compile_syntax_error(source: &str) -> String {
    let runtime = Runtime::new_with_host_services(quickjs_oxide_host::SystemHost::default());
    let context = runtime.new_context().expect("create context");
    let error = match context.compile(source) {
        Err(RuntimeError::Exception(Value::Object(error))) => error,
        Err(RuntimeError::Exception(_)) => {
            panic!("compile exception was not an Error object: {source:?}")
        }
        _ => panic!("source did not fail with a JavaScript exception: {source:?}"),
    };
    assert!(runtime.is_error_object(&error).unwrap());

    let name_key = runtime.intern_property_key("name").unwrap();
    let Value::String(name) = context.get_property(&error, &name_key).unwrap() else {
        panic!("compile exception name was not a string: {source:?}");
    };
    assert_eq!(name.to_utf8_lossy(), "SyntaxError", "{source}");

    let message_key = runtime.intern_property_key("message").unwrap();
    let Value::String(message) = context.get_property(&error, &message_key).unwrap() else {
        panic!("compile exception message was not a string: {source:?}");
    };
    message.to_utf8_lossy()
}
