use crate::object_graph_observation::data_property as data_descriptor;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{Context, ObjectRef, Runtime, RuntimeError, Value};

#[test]
fn typed_array_entries_pair_and_errors_use_the_builtin_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let defining_array_iterator_prototype = eval_object(
        &mut defining,
        "Object.getPrototypeOf([].values())",
        "defining Array Iterator prototype",
    );
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_array_prototype = caller.array_prototype().unwrap();
    let entries = eval_callable(
        &runtime,
        &mut defining,
        "Object.getPrototypeOf(Uint8Array.prototype).entries",
        "defining TypedArray entries",
    );

    let receiver = eval_object(
        &mut caller,
        "new Uint8Array([41,42])",
        "caller TypedArray receiver",
    );
    let Value::Object(iterator) = caller
        .call(&entries, Value::Object(receiver), &[])
        .expect("cross-realm TypedArray entries call")
    else {
        panic!("TypedArray entries did not return an iterator object");
    };
    assert_eq!(
        runtime.get_prototype_of(&iterator).unwrap(),
        Some(defining_array_iterator_prototype),
        "TypedArray entries allocated its iterator outside the builtin defining realm",
    );

    let next = property_callable(&runtime, &mut caller, &iterator, "next");
    let Value::Object(result) = caller
        .call(&next, Value::Object(iterator), &[])
        .expect("cross-realm TypedArray entries next")
    else {
        panic!("TypedArray entries next did not return an iterator result object");
    };
    let pair = object_property(&runtime, &mut caller, &result, "value");
    assert_eq!(
        runtime.get_prototype_of(&pair).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "TypedArray entries pair did not use the builtin defining realm Array",
    );
    assert_ne!(
        runtime.get_prototype_of(&pair).unwrap(),
        Some(caller_array_prototype.try_clone().expect("duplicate root")),
        "TypedArray entries pair leaked into the caller realm",
    );

    // QuickJS's JS_IteratorNext2 fast path calls the foreign native `next`
    // pointer with the outer operation context. A direct `.next()` above
    // therefore uses the builtin realm, while caller bytecode consuming the
    // same kind of iterator must allocate entries pairs in the caller realm.
    let receiver = eval_object(
        &mut caller,
        "new Uint8Array([43])",
        "caller TypedArray fast-path receiver",
    );
    let Value::Object(foreign_iterator) = caller
        .call(&entries, Value::Object(receiver), &[])
        .expect("cross-realm TypedArray entries fast-path call")
    else {
        panic!("TypedArray entries fast-path probe did not return an iterator object");
    };
    let foreign_key = runtime.intern_property_key("__foreignEntries").unwrap();
    assert!(
        caller
            .define_own_property(
                &caller.global_object().unwrap(),
                &foreign_key,
                &data_descriptor(Value::Object(foreign_iterator)),
            )
            .unwrap()
    );
    let spread = eval_object(
        &mut caller,
        "[...__foreignEntries]",
        "caller spread of foreign TypedArray entries",
    );
    let caller_pair = object_property(&runtime, &mut caller, &spread, "0");
    assert_eq!(
        runtime.get_prototype_of(&caller_pair).unwrap(),
        Some(caller_array_prototype),
        "foreign TypedArray entries fast path did not keep the outer operation realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&caller_pair).unwrap(),
        Some(defining_array_prototype),
        "foreign TypedArray entries fast path reused the next builtin realm",
    );

    let ordinary = eval_object(&mut caller, "({})", "caller ordinary receiver");
    assert!(matches!(
        caller.call(&entries, Value::Object(ordinary), &[]),
        Err(RuntimeError::Exception),
    ));
    let error = take_exception_object(&mut caller, "cross-realm TypedArray brand TypeError");
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(defining_type_error),
        "TypedArray iterator brand TypeError did not use the builtin defining realm",
    );
}

fn object_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> ObjectRef {
    let key = runtime.intern_property_key(name).unwrap();
    let Value::Object(value) = context
        .get_property(object, &key)
        .unwrap_or_else(|error| panic!("read object property {name}: {error}"))
    else {
        panic!("{name} was not an object");
    };
    value
}
