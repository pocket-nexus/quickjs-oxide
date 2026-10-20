use crate::runtime_observation::host_runtime_context;

use quickjs_oxide::engine::api::core::{JsString, Value};

// This target pins QuickJS 2026-06-04 `js_array_slice`, its magic-selected
// mutating `splice` branch, and the adjacent dense `js_array_toSpliced` path.

#[test]
fn array_slice_splice_basic_rust_smoke() {
    let (_runtime, context) = host_runtime_context();
    assert_eq!(
        context
            .eval(
                r#"(function(){
                    var source=[1,2,3,4],slice=source.slice(1,3);
                    var removed=source.splice(1,2,8,9,10),copy=source.toSpliced(1,1,7);
                    return slice.join(",")+"|"+removed.join(",")+"|"+
                        source.join(",")+"|"+copy.join(",");
                })()"#,
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("2,3|2,3|1,8,9,10,4|1,7,9,10,4").unwrap()),
    );
}

#[test]
fn array_slice_recursive_getter_stack_overflow_is_catchable_without_oracle() {
    let (_runtime, context) = host_runtime_context();
    assert_eq!(
        context
            .eval(
                r#"(function(){
                    var source=Object();source.length=1;
                    source.__defineGetter__("0",function(){
                        return Array.prototype.slice.call(source,0,1)[0];
                    });
                    try{Array.prototype.slice.call(source,0,1);return "missing"}
                    catch(error){return error.name+"|"+error.message}
                })()"#,
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("InternalError|stack overflow").unwrap()),
    );
}
