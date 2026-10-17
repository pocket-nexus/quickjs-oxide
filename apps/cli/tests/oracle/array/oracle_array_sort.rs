use super::support::*;
use crate::runtime_observation::host_runtime;

use quickjs_oxide::engine::api::core::Value;

// This target pins QuickJS 2026-06-04's ordinary Array sort kernel and its
// change-by-copy wrapper. Stateful probes intentionally lock rqsort comparison
// order and QuickJS's write-back shortcuts, not merely the final permutation.

#[test]
fn array_sort_atom_literal_identity_survives_publication_and_context_boundaries() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
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
    let array_prototype = defining.array_prototype().unwrap();
    let sort = property_callable(&runtime, &mut defining, &array_prototype, "sort");
    drop(caller.eval("var sortCalls=0").unwrap());
    let comparator = eval_callable(
        &runtime,
        &mut caller,
        "(function(){sortCalls++;return 0})",
        "literal identity comparator",
    );
    let comparator = Value::Object(comparator.as_object().try_clone().expect("duplicate root"));

    let first = defining.eval("'shared across contexts'").unwrap();
    let second = caller.eval("'shared across contexts'").unwrap();
    let receiver = caller.new_array_from_values(vec![first, second]).unwrap();
    drop(
        caller
            .call(
                &sort,
                Value::Object(receiver),
                std::slice::from_ref(&comparator),
            )
            .unwrap(),
    );
    assert_eq!(caller.eval("sortCalls").unwrap(), Value::Int(0));

    drop(caller.eval("sortCalls=0").unwrap());
    let literal = caller.eval("'1'").unwrap();
    let property = runtime.intern_property_key("1").unwrap();
    let property = Value::String(runtime.property_key_to_js_string(&property).unwrap());
    let receiver = caller
        .new_array_from_values(vec![literal, property])
        .unwrap();
    drop(
        caller
            .call(
                &sort,
                Value::Object(receiver),
                std::slice::from_ref(&comparator),
            )
            .unwrap(),
    );
    assert_eq!(caller.eval("sortCalls").unwrap(), Value::Int(1));

    drop(caller.eval("sortCalls=0").unwrap());
    let literal = caller.eval("'shared property spelling'").unwrap();
    let property = runtime
        .intern_property_key("shared property spelling")
        .unwrap();
    let property = Value::String(runtime.property_key_to_js_string(&property).unwrap());
    let receiver = caller
        .new_array_from_values(vec![literal, property])
        .unwrap();
    drop(
        caller
            .call(
                &sort,
                Value::Object(receiver),
                std::slice::from_ref(&comparator),
            )
            .unwrap(),
    );
    assert_eq!(caller.eval("sortCalls").unwrap(), Value::Int(0));

    drop(caller.eval("sortCalls=0").unwrap());
    let first = defining.eval("'1'").unwrap();
    let second = caller.eval("'1'").unwrap();
    let receiver = caller.new_array_from_values(vec![first, second]).unwrap();
    drop(
        caller
            .call(&sort, Value::Object(receiver), &[comparator])
            .unwrap(),
    );
    assert_eq!(caller.eval("sortCalls").unwrap(), Value::Int(1));
}
