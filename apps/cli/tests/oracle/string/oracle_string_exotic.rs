use crate::object_graph_observation::data_descriptor;
use crate::object_graph_observation::eval_callable;
use crate::object_graph_observation::expect_object;
use crate::runtime_observation::host_runtime;
use quickjs_oxide::engine::api::{Context, JsString, ObjectRef, PropertyKey, Runtime, Value};

#[test]
fn sloppy_string_boxing_uses_the_bytecode_functions_defining_realm() {
    let runtime = host_runtime();
    let mut first = runtime.new_context().expect("create context");
    let mut second = runtime.new_context().expect("create context");
    let first_boxer = eval_callable(&runtime, &mut first, "(function () { return this; })");
    let second_boxer = eval_callable(&runtime, &mut second, "(function () { return this; })");
    let payload = JsString::try_from_utf16([0x41, 0xd800]).unwrap();

    let first_wrapper = expect_object(
        second
            .call(&first_boxer, Value::String(payload.clone()), &[])
            .unwrap(),
        "foreign call to first-realm sloppy boxer",
    );
    let first_wrapper_again = expect_object(
        first
            .call(&first_boxer, Value::String(payload.clone()), &[])
            .unwrap(),
        "local call to first-realm sloppy boxer",
    );
    let second_wrapper = expect_object(
        first
            .call(&second_boxer, Value::String(payload), &[])
            .unwrap(),
        "foreign call to second-realm sloppy boxer",
    );
    let first_prototype = runtime.get_prototype_of(&first_wrapper).unwrap().unwrap();
    let first_prototype_again = runtime
        .get_prototype_of(&first_wrapper_again)
        .unwrap()
        .unwrap();
    let second_prototype = runtime.get_prototype_of(&second_wrapper).unwrap().unwrap();

    assert_eq!(first_prototype, first_prototype_again);
    assert_ne!(first_prototype, second_prototype);
    let length = runtime.intern_property_key("length").unwrap();
    assert_eq!(
        data_descriptor(&runtime, &first_prototype, &length),
        (Value::Int(0), false, false, true)
    );
    assert_eq!(
        data_descriptor(&runtime, &second_prototype, &length),
        (Value::Int(0), false, false, true)
    );
}

#[test]
fn rooted_string_wrapper_preserves_payload_and_final_release_collects_the_graph() {
    let runtime = host_runtime();
    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        box_string(
            &runtime,
            &mut context,
            JsString::try_from_utf16([0x41, 0xd83d, 0xde00, 0xd800]).unwrap(),
        )
    };

    runtime.run_gc().unwrap();
    let index = runtime.intern_property_key("3").unwrap();
    assert_eq!(
        data_descriptor(&runtime, &wrapper, &index),
        (
            Value::String(JsString::try_from_utf16([0xd800]).unwrap()),
            false,
            true,
            false,
        ),
        "the rooted wrapper must preserve its exact UTF-16 payload across GC"
    );
    assert_eq!(
        own_key_names(&runtime, &wrapper, &[]),
        ["0", "1", "2", "3", "length"]
    );

    // The String prototype now publishes the forty-five-key partial surface;
    // its saved-method realm edge is tested by the UTF-16-prefix oracle. This
    // test isolates wrapper/payload survival and final cleanup.
    drop(wrapper);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}

fn box_string(runtime: &Runtime, context: &mut Context, value: JsString) -> ObjectRef {
    let boxer = eval_callable(runtime, context, "(function () { return this; })");
    expect_object(
        context.call(&boxer, Value::String(value), &[]).unwrap(),
        "sloppy String this boxing",
    )
}

fn own_key_names(
    runtime: &Runtime,
    object: &ObjectRef,
    symbols: &[(&PropertyKey, &str)],
) -> Vec<String> {
    runtime
        .own_property_keys(object)
        .unwrap()
        .iter()
        .map(|key| {
            symbols
                .iter()
                .find_map(|(symbol, name)| (key == *symbol).then(|| (*name).to_owned()))
                .unwrap_or_else(|| {
                    runtime
                        .property_key_to_js_string(key)
                        .unwrap()
                        .to_utf8_lossy()
                })
        })
        .collect()
}
