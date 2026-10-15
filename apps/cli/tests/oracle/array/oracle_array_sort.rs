use super::support::*;
use crate::runtime_observation::host_runtime;

use crate::object_graph_observation::int_property;
use quickjs_oxide::engine::api::{RuntimeError, Value};

// This target pins QuickJS 2026-06-04's ordinary Array sort kernel and its
// change-by-copy wrapper. Stateful probes intentionally lock rqsort comparison
// order and QuickJS's write-back shortcuts, not merely the final permutation.

#[test]
fn array_sort_cross_realm_results_boxing_native_and_user_errors_match_quickjs() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let caller_array_prototype = caller.array_prototype().unwrap();
    let defining_number_prototype = eval_object(
        &mut defining,
        "Number.prototype",
        "defining Number prototype",
    );
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let defining_range_error = eval_object(
        &mut defining,
        "RangeError.prototype",
        "defining RangeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    let sort = property_callable(&runtime, &mut defining, &defining_array_prototype, "sort");
    let to_sorted = property_callable(
        &runtime,
        &mut defining,
        &defining_array_prototype,
        "toSorted",
    );

    let Value::Object(boxed) = caller
        .call(&sort, Value::Int(7), &[])
        .expect("cross-realm primitive Array.sort")
    else {
        panic!("cross-realm primitive Array.sort did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&boxed).unwrap(),
        Some(defining_number_prototype),
        "Array.sort boxed outside the method defining realm",
    );

    let receiver = eval_object(&mut caller, "[3,1,2]", "caller Array receiver");
    let Value::Object(result) = caller
        .call(&to_sorted, Value::Object(receiver), &[])
        .expect("cross-realm Array.toSorted")
    else {
        panic!("cross-realm Array.toSorted did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "Array.toSorted result did not use the method defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype),
    );
    assert_eq!(int_property(&runtime, &mut caller, &result, "0"), 1);
    assert_eq!(int_property(&runtime, &mut caller, &result, "2"), 3);

    assert!(matches!(
        caller.call(&sort, Value::Null, &[Value::Int(1)]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.sort comparefn TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "Array.sort comparefn TypeError did not use the method defining realm",
    );

    let too_long = eval_object(
        &mut caller,
        "(function(){var source=Object();source.length=2147483648;return source})()",
        "caller oversized array-like",
    );
    assert!(matches!(
        caller.call(&to_sorted, Value::Object(too_long), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.toSorted RangeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_range_error),
        "Array.toSorted RangeError did not use the method defining realm",
    );

    let fixed_tail = eval_object(
        &mut caller,
        "(function(){var source=Object(),descriptor=Object();source.length=2;descriptor.value='tail';descriptor.writable=true;descriptor.enumerable=true;descriptor.configurable=false;Object.defineProperty(source,'1',descriptor);return source})()",
        "caller non-configurable sort tail",
    );
    assert!(matches!(
        caller.call(&sort, Value::Object(fixed_tail), &[]),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.sort Delete TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.sort Delete TypeError did not use the method defining realm",
    );

    let comparator = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw new TypeError('caller comparator')})",
        "caller throwing comparator",
    );
    let receiver = eval_object(&mut caller, "[2,1]", "caller comparator receiver");
    assert!(matches!(
        caller.call(
            &sort,
            Value::Object(receiver),
            &[Value::Object(
                comparator.as_object().try_clone().expect("duplicate root")
            )],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.sort user comparator TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.sort replaced a user comparator throw with a defining-realm error",
    );
}

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
            quickjs_oxide::engine::api::JsString::try_from_utf8("0|0|1|0|0|0|1|1").unwrap(),
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
