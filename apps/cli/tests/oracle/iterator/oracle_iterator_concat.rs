use crate::object_graph_observation::checked_property_value as object_property;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::eval_object;
use quickjs_oxide::engine::api::{RuntimeError, Value};

#[test]
fn iterator_concat_cross_realm_graph_and_native_next_use_the_current_realm() {
    // The qjs CLI cannot bridge objects between two contexts. This directly
    // pins the corresponding QuickJS C path: the concat iterator and public
    // iterator-result wrappers use the concat intrinsic's defining context,
    // while JS_IteratorNext2 invokes an inner IteratorNext cproto with that
    // current context rather than switching to the inner method's realm. A
    // same-runtime, two-context probe against the pinned libquickjs reports:
    // `sequence-prototype=1:0|step-prototype=1|value-identity=1|native-error=1:0`.
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let iterator = eval_object(&mut defining, "Iterator", "defining Iterator");
    let concat = property_callable(&runtime, &mut defining, &iterator, "concat");
    let defining_concat_prototype = eval_object(
        &mut defining,
        "Object.getPrototypeOf(Iterator.concat())",
        "defining concat prototype",
    );
    let caller_concat_prototype = eval_object(
        &mut caller,
        "Object.getPrototypeOf(Iterator.concat())",
        "caller concat prototype",
    );
    let defining_object_prototype = defining.object_prototype().unwrap();
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );

    drop(
        caller
            .eval("globalThis.__qjoConcatMarker=Object()")
            .expect("install caller concat marker"),
    );
    let marker = eval_object(
        &mut caller,
        "globalThis.__qjoConcatMarker",
        "caller concat marker",
    );
    let iterable = eval_object(
        &mut caller,
        "[globalThis.__qjoConcatMarker]",
        "caller iterable",
    );
    let Value::Object(sequence) = caller
        .call(&concat, Value::Undefined, &[Value::Object(iterable)])
        .expect("cross-realm Iterator.concat call")
    else {
        panic!("cross-realm Iterator.concat did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&sequence).unwrap(),
        Some(defining_concat_prototype),
        "Iterator.concat used the caller realm's hidden concat prototype",
    );
    assert_ne!(
        runtime.get_prototype_of(&sequence).unwrap(),
        Some(caller_concat_prototype),
    );

    let next = property_callable(&runtime, &mut caller, &sequence, "next");
    let Value::Object(step) = caller
        .call(&next, Value::Object(sequence), &[])
        .expect("cross-realm concat next")
    else {
        panic!("cross-realm concat next did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&step).unwrap(),
        Some(defining_object_prototype),
        "concat next did not allocate its public result in the defining operation realm",
    );
    assert_eq!(
        object_property(&runtime, &mut caller, &step, "value"),
        Value::Object(marker),
        "concat next did not preserve the caller-realm yielded object",
    );
    assert_eq!(
        object_property(&runtime, &mut caller, &step, "done"),
        Value::Bool(false),
    );

    // This iterable returns an ordinary object whose `next` is the caller
    // realm's native Array Iterator next method. The receiver fails that
    // method's brand. A generic native call would create caller TypeError;
    // the raw IteratorNext path must create defining TypeError instead.
    let invalid_iterable = eval_object(
        &mut caller,
        r#"(function(){
            var nativeNext=
                Object.getPrototypeOf([][Symbol.iterator]()).next;
            return {
                [Symbol.iterator]:function(){
                    return {next:nativeNext};
                }
            };
        })()"#,
        "caller iterable with wrong-brand native next",
    );
    let Value::Object(invalid_sequence) = caller
        .call(
            &concat,
            Value::Undefined,
            &[Value::Object(invalid_iterable)],
        )
        .expect("construct wrong-brand cross-realm concat")
    else {
        panic!("wrong-brand cross-realm concat was not an object");
    };
    let invalid_next = property_callable(&runtime, &mut caller, &invalid_sequence, "next");
    assert!(matches!(
        caller.call(&invalid_next, Value::Object(invalid_sequence), &[],),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(
        &mut caller,
        "cross-realm concat native IteratorNext TypeError",
    );
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error),
        "concat's raw native IteratorNext path switched to the inner method realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_type_error),
    );
}
