use crate::object_graph_observation::global_callable;
use crate::runtime_observation::host_runtime;

#[test]
fn global_uri_codec_keeps_its_defining_realm_alive_until_collection() {
    let runtime = host_runtime();
    let codec = {
        let mut context = runtime.new_context().expect("create context");
        global_callable(&runtime, &mut context, "encodeURIComponent")
    };

    runtime.run_gc().unwrap();
    drop(codec);
    runtime.run_gc().unwrap();
}
