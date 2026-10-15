use crate::runtime_completion_oracle::run;

#[test]
fn async_generator_object_method_smoke_uses_the_independent_driver() {
    const SOURCE: &str = r#"
var object = {
    async *method() {
        yield 42;
    }
};
object.method().next().then(function (result) {
    print(result.value);
});
"#;

    let oxide = run(env!("CARGO_BIN_EXE_qjs").as_ref(), SOURCE);
    assert!(
        oxide.status.success(),
        "quickjs-oxide rejected the async-generator object method smoke: {}",
        String::from_utf8_lossy(&oxide.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&oxide.stdout), "42\n");

    let oracle = require_pinned_qjs_oracle!("async-generator object-method smoke oracle check");
    let quickjs = run(&oracle, SOURCE);
    assert!(
        quickjs.status.success(),
        "pinned QuickJS rejected the async-generator object-method smoke: {}",
        String::from_utf8_lossy(&quickjs.stderr)
    );
    assert_eq!(quickjs.stdout, oxide.stdout);
}
