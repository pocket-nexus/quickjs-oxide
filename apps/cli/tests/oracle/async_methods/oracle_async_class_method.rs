use crate::runtime_completion_oracle::run;

#[test]
fn public_async_generator_class_method_smoke_is_admitted() {
    let source = r#"
class C {
    async *method() { yield 42; }
}
new C().method().next().then(function (result) {
    print(result.value);
});
"#;

    let oracle = std::env::var_os("QJS_ORACLE");
    if oracle.is_none() {
        eprintln!(
            "SKIP public async-generator class method smoke oracle check: \
             set QJS_ORACLE to pinned upstream qjs"
        );
    }
    let oxide = run(env!("CARGO_BIN_EXE_qjs").as_ref(), source);
    assert!(
        oxide.status.success(),
        "quickjs-oxide rejected the public async-generator class method: {}",
        String::from_utf8_lossy(&oxide.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&oxide.stdout), "42\n");

    if let Some(oracle) = &oracle {
        let quickjs = run(oracle, source);
        assert!(
            quickjs.status.success(),
            "pinned QuickJS rejected the recorded async-generator frontier: {}",
            String::from_utf8_lossy(&quickjs.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&quickjs.stdout), "42\n");
    }
}
