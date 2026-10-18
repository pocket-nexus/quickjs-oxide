use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::core::Value;

// This target pins QuickJS 2026-06-04's ordinary Array sort kernel and its
// change-by-copy wrapper. Stateful probes intentionally lock rqsort comparison
// order and QuickJS's write-back shortcuts, not merely the final permutation.

#[test]
fn array_sort_atom_literal_identity_matches_quickjs() {
    // QuickJS calls no comparator for two identical atoms. The embedding
    // API copies strings across its boundary (new-core phase 6, 2.7), so
    // only the identities JavaScript itself makes are observable.
    let runtime = host_runtime();
    let mut caller = runtime.new_context().expect("create context");
    assert_eq!(
        caller
            .eval(
                r#"(function(){
                    function calls(a,b){var count=0;[a,b].sort(function(){count++;return 0});return count}
                    function left(){return "same"}function right(){return "same"}
                    function numeric(){return "1"}
                    var a="sa",b="me";
                    return calls("same","same")+"|"+calls(left(),right())+"|"+
                        calls("1","1")+"|"+calls(numeric(),numeric())+"|"+
                        calls("2147483648","2147483648")+"|"+calls(`same`,`same`)+"|"+
                        calls(`1`,`1`)+"|"+calls(a+b,a+b);
                })()"#,
            )
            .unwrap(),
        Value::String(
            quickjs_oxide::engine::api::core::JsString::try_from_utf8("0|0|1|0|0|0|1|1").unwrap(),
        ),
    );
}
