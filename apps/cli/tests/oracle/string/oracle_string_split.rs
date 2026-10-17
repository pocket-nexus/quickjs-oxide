use crate::object_graph_observation::int_property;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::property_callable;
use quickjs_oxide::engine::api::core::{JsString, Value};

// Differential lock for pinned QuickJS 2026-06-04 `js_string_split`
// (`quickjs.c` 45894-45980) and its prototype-table entry (46640).
//
// The JavaScript vectors intentionally stay inside quickjs-oxide's implemented
// grammar. In particular they do not use RegExp literals or constructors,
// object-literal methods/accessors, classes, arrows, destructuring, modules, or
// async syntax. RegExp integration belongs to RegExp.prototype[Symbol.split];
// this gate covers the complete generic String method, including arbitrary
// @@split delegation.

#[test]
fn detached_string_split_and_result_retain_then_release_their_defining_realm() {
    let runtime = host_runtime();
    let split = {
        let mut defining = runtime.new_context().expect("create context");
        let prototype = defining.string_prototype().unwrap();
        property_callable(&runtime, &mut defining, &prototype, "split")
    };
    runtime.run_gc().unwrap();

    let result = {
        let mut caller = runtime.new_context().expect("create context");
        let Value::Object(result) = caller
            .call(
                &split,
                Value::String(JsString::try_from_utf8("x-y").unwrap()),
                &[Value::String(JsString::try_from_utf8("-").unwrap())],
            )
            .expect("detached split call")
        else {
            panic!("detached split result was not an object");
        };
        assert_eq!(int_property(&runtime, &mut caller, &result, "length"), 2);
        result
    };
    drop(split);
    runtime.run_gc().unwrap();
    drop(result);
    runtime.run_gc().unwrap();
}
