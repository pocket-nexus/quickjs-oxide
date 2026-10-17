use crate::object_graph_observation::own_key_names;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::core::{CallableRef, Context, JsString, ObjectRef, Value};

#[test]
fn string_conversion_core_wrapper_realm_graph_is_collectable() {
    let runtime = host_runtime();
    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let object = context.object_prototype().unwrap();
        let ovo = property_callable(&runtime, &mut context, &object, "valueOf");
        box_string(
            &mut context,
            &ovo,
            JsString::try_from_utf16([0x41, 0xd800, 0x42]).unwrap(),
        )
    };
    runtime.run_gc().unwrap();
    assert_eq!(own_key_names(&runtime, &wrapper), ["0", "1", "2", "length"]);
    drop(wrapper);
    runtime.run_gc().unwrap();
}

fn box_string(context: &mut Context, ovo: &CallableRef, value: JsString) -> ObjectRef {
    let Value::Object(object) = context.call(ovo, Value::String(value), &[]).unwrap() else {
        panic!("Object.prototype.valueOf did not box String");
    };
    object
}
