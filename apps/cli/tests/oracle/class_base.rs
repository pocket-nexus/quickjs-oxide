use crate::common::compile_syntax_error;
use crate::runtime_observation::host_runtime_context;
use quickjs_oxide::engine::api::core::Value;

#[test]
fn public_async_generator_class_method_smoke_is_admitted() {
    let source = "class C { async *method() {} }";
    let (_runtime, context) = host_runtime_context();
    context
        .compile(source)
        .expect("public async-generator class methods should compile");
}

#[test]
fn base_class_early_errors_are_rejected_during_compilation() {
    for (source, expected) in [
        (
            "class C { constructor() {} 'constructor'() {} }",
            "constructor appears more than once",
        ),
        ("class C { get constructor() {} }", "invalid method name"),
        (
            "class C { set 'constructor'(value) {} }",
            "invalid method name",
        ),
        ("class C { static prototype() {} }", "invalid method name"),
        ("class {}", "class statement requires a name"),
        (
            "if (true) class C {}",
            "class declarations can't appear in single-statement context",
        ),
    ] {
        let message = compile_syntax_error(source);
        assert!(message.contains(expected), "{source}: {message}");
    }
}

#[test]
fn class_expression_restores_outer_lexing_and_asi_trivia() {
    let (_runtime, context) = host_runtime_context();
    let Value::String(name) = context
        .eval("var AsiClass = class AsiClass {}\nAsiClass.name")
        .expect("class expression followed by ASI should compile and run")
    else {
        panic!("class ASI probe did not return a String");
    };
    assert_eq!(name.to_utf8_lossy(), "AsiClass");
}
