use super::*;

#[test]
fn direct_eval_identity_is_realm_local_and_independent_of_the_global_property() {
    let runtime = Runtime::new();
    let mut first = runtime.new_context();
    let mut second = runtime.new_context();
    let first_eval = global_callable(&runtime, &mut first, "eval");
    let second_eval = global_callable(&runtime, &mut second, "eval");
    let first_value = Value::Object(first_eval.as_object().clone());
    let second_value = Value::Object(second_eval.as_object().clone());

    assert!(runtime.is_original_eval(first.realm, &first_value).unwrap());
    assert!(
        !runtime
            .is_original_eval(first.realm, &second_value)
            .unwrap()
    );

    assert!(
        runtime
            .is_original_eval(second.realm, &second_value)
            .unwrap()
    );
    assert!(
        !runtime
            .is_original_eval(second.realm, &first_value)
            .unwrap()
    );

    let foreign_runtime = Runtime::new();
    let mut foreign_context = foreign_runtime.new_context();
    let foreign_eval = global_callable(&foreign_runtime, &mut foreign_context, "eval");
    assert_eq!(
        runtime.is_original_eval(
            first.realm,
            &Value::Object(foreign_eval.as_object().clone())
        ),
        Err(RuntimeError::WrongRuntime("eval function"))
    );

    assert_eq!(
        second.eval("delete globalThis.eval").unwrap(),
        Value::Bool(true)
    );
    assert!(
        runtime
            .is_original_eval(second.realm, &second_value)
            .unwrap()
    );
    second
        .eval("globalThis.eval = function replacement() { return 17; }")
        .unwrap();
    let replacement = global_callable(&runtime, &mut second, "eval");
    assert!(
        !runtime
            .is_original_eval(
                second.realm,
                &Value::Object(replacement.as_object().clone())
            )
            .unwrap()
    );
    assert!(
        runtime
            .is_original_eval(second.realm, &second_value)
            .unwrap()
    );
}
