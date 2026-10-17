use crate::object_graph_observation::global_callable;
use crate::runtime_observation::host_runtime;

#[test]
fn global_numeric_predicate_keeps_its_defining_realm_alive_until_collection() {
    let runtime = host_runtime();
    let predicate = {
        let mut context = runtime.new_context().expect("create context");
        global_callable(&runtime, &mut context, "isNaN")
    };

    runtime.run_gc().unwrap();
    drop(predicate);
    runtime.run_gc().unwrap();
}
