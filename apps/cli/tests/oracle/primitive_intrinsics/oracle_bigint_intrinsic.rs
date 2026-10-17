use crate::object_graph_observation::expect_object;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::core::{JsBigInt, Value};

#[test]
fn bigint_wrapper_keeps_its_realm_graph_alive_until_collection() {
    let runtime = host_runtime();
    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let object_prototype = context.object_prototype().unwrap();
        let object_value_of =
            property_callable(&runtime, &mut context, &object_prototype, "valueOf");
        expect_object(
            context
                .call(&object_value_of, Value::BigInt(JsBigInt::from(123)), &[])
                .unwrap(),
            "Object.prototype.valueOf BigInt wrapper",
        )
    };

    runtime.run_gc().unwrap();
    drop(wrapper);
    runtime.run_gc().unwrap();
}
