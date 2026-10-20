use crate::runtime_observation::host_runtime;
use quickjs_oxide::engine::api::core::{DebugInfoMode, Value};

#[test]
fn tagged_template_site_identity_survives_gc_in_strip_debug_mode() {
    let runtime = host_runtime();
    runtime
        .set_debug_info_mode(DebugInfoMode::StripDebug)
        .expect("set runtime configuration");
    let context = runtime.new_context().expect("create context");
    drop(
        context
            .eval(
                r#"
                function __qjoTaggedGcTag(strings) { return strings; }
                function __qjoTaggedGcSite() { return __qjoTaggedGcTag`alive`; }
            "#,
            )
            .expect("compile StripDebug tagged-template GC fixture"),
    );

    let Value::Object(first) = context
        .eval("__qjoTaggedGcSite()")
        .expect("evaluate tagged-template site before GC")
    else {
        panic!("tagged-template site did not return its template object before GC");
    };
    let raw_key = runtime
        .intern_property_key("raw")
        .expect("template raw property key");
    let Value::Object(first_raw) = context
        .get_property(&first, &raw_key)
        .expect("read template raw array before GC")
    else {
        panic!("template raw property was not an object before GC");
    };

    runtime.run_gc().expect("collect tagged-template fixture");

    let Value::Object(second) = context
        .eval("__qjoTaggedGcSite()")
        .expect("evaluate tagged-template site after GC")
    else {
        panic!("tagged-template site did not return its template object after GC");
    };
    let Value::Object(second_raw) = context
        .get_property(&second, &raw_key)
        .expect("read template raw array after GC")
    else {
        panic!("template raw property was not an object after GC");
    };

    assert_eq!(first, second, "template object identity changed across GC");
    assert_eq!(
        first_raw, second_raw,
        "template raw array identity changed across GC"
    );
}
