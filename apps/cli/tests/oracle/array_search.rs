use crate::runtime_observation::host_runtime;

use crate::runtime_observation::property_callable_with_read_context as property_callable;
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// This differential pins the first Array.prototype algorithm slice after the
// iterator surface to QuickJS 2026-06-04. Source probes avoid later Array
// methods and later Object reflection APIs on the Rust side. The graph probe
// uses the host API so own-key order and lazy native metadata remain observable
// without pulling Object.getOwnPropertyDescriptor into this milestone.

#[test]
fn array_search_errors_use_the_native_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    drop(
        defining
            .eval("TypeError.prototype.arraySearchRealm='defining'")
            .expect("mark defining TypeError prototype"),
    );
    drop(
        caller
            .eval("TypeError.prototype.arraySearchRealm='caller'")
            .expect("mark caller TypeError prototype"),
    );
    let defining_array_prototype = defining.array_prototype().unwrap();
    let marker_key = runtime.intern_property_key("arraySearchRealm").unwrap();

    for name in ["at", "includes", "indexOf", "lastIndexOf"] {
        let method = property_callable(&runtime, &mut defining, &defining_array_prototype, name);
        assert!(matches!(
            caller.call(&method, Value::Null, &[Value::Undefined]),
            Err(RuntimeError::Exception),
        ));
        let Value::Object(error) = caller
            .take_exception()
            .unwrap_or_else(|failure| panic!("take cross-realm {name} exception: {failure}"))
            .unwrap_or_else(|| panic!("cross-realm {name} exception was missing"))
        else {
            panic!("cross-realm {name} exception was not an object");
        };
        assert_eq!(
            caller.get_property(&error, &marker_key).unwrap(),
            Value::String(JsString::try_from_utf8("defining").unwrap()),
            "{name} allocated its TypeError in the caller realm",
        );
    }
}
