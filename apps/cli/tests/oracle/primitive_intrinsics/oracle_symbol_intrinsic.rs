use crate::object_graph_observation::expect_object;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::core::{CallableRef, Context, JsString, SymbolRef, Value};

#[test]
fn symbol_primitives_do_not_retain_realms_but_wrappers_do_and_atoms_survive() {
    let runtime = host_runtime();
    let primitive = {
        let mut context = runtime.new_context().expect("create context");
        let global = context.global_object().unwrap();
        let constructor = property_callable(&runtime, &mut context, &global, "Symbol");
        call_symbol(
            &mut context,
            &constructor,
            &[Value::String(
                JsString::try_from_utf16([0x61, 0xd800, 0x62]).unwrap(),
            )],
        )
    };
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.symbol_description(&primitive).unwrap(),
        Some(JsString::try_from_utf16([0x61, 0xd800, 0x62]).unwrap()),
        "the symbol atom must outlive its creating context while rooted"
    );
    drop(primitive);
    runtime.run_gc().unwrap();

    let wrapper = {
        let mut context = runtime.new_context().expect("create context");
        let object_prototype = context.object_prototype().unwrap();
        let object_value_of =
            property_callable(&runtime, &mut context, &object_prototype, "valueOf");
        let symbol = runtime
            .new_symbol(Some(JsString::try_from_utf8("wrapper-only").unwrap()))
            .unwrap();
        expect_object(
            context
                .call(&object_value_of, Value::Symbol(symbol), &[])
                .unwrap(),
            "Object.prototype.valueOf Symbol wrapper",
        )
    };
    runtime.run_gc().unwrap();
    drop(wrapper);
    runtime.run_gc().unwrap();
}

fn call_symbol(context: &mut Context, callable: &CallableRef, arguments: &[Value]) -> SymbolRef {
    let Value::Symbol(value) = context.call(callable, Value::Undefined, arguments).unwrap() else {
        panic!("Symbol-producing call did not return a Symbol");
    };
    value
}
