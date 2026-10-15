use crate::runtime_observation::host_runtime;
use crate::runtime_observation::string_value;
use crate::runtime_observation::take_exception_object;
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04 `js_string_match`'s
// Symbol.search branch (`quickjs.c` 45609-45657), abstract RegExpExec
// (`quickjs.c` 48217-48236), and RegExp.prototype[Symbol.search]
// (`quickjs.c` 48817-48873).

#[test]
fn search_intrinsics_use_defining_realms_and_accept_foreign_regexp_brands() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let string_search = eval_callable(
        &runtime,
        &mut defining,
        "String.prototype.search",
        "defining String search",
    );
    let regexp_search = eval_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype[Symbol.search]",
        "defining RegExp Symbol.search",
    );
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    assert_ne!(defining_type_error, caller_type_error);

    drop(
        defining
            .eval("RegExp.prototype[Symbol.search]=function(){return 41}")
            .unwrap(),
    );
    drop(
        caller
            .eval("RegExp.prototype[Symbol.search]=function(){return 99}")
            .unwrap(),
    );
    assert_eq!(
        caller
            .call(
                &string_search,
                string_value("subject"),
                &[string_value("needle")],
            )
            .unwrap(),
        Value::Int(41),
        "String search fallback did not use its defining-realm RegExp intrinsic",
    );

    let foreign_regexp = eval_object(
        &mut caller,
        "(function(){var r=/b/g;r.lastIndex=7;r.exec=null;return r})()",
        "caller RegExp with builtin exec fallback",
    );
    assert_eq!(
        caller
            .call(
                &regexp_search,
                Value::Object(foreign_regexp.try_clone().expect("duplicate root")),
                &[string_value("abc")],
            )
            .unwrap(),
        Value::Int(1),
        "foreign branded RegExp did not pass defining-realm builtin fallback",
    );
    let last_index_key = runtime.intern_property_key("lastIndex").unwrap();
    assert_eq!(
        caller
            .get_property(&foreign_regexp, &last_index_key)
            .unwrap(),
        Value::Int(7),
        "foreign RegExp lastIndex was not restored",
    );

    assert_eq!(
        caller.call(&regexp_search, Value::Int(1), &[string_value("x")]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "defining-realm search TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "native RegExp Symbol.search TypeError used the caller realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var receiver=Object();receiver.lastIndex=0;
            receiver.exec=function(){throw new TypeError("caller exec")};
            return receiver;
        })()"#,
        "caller throwing custom exec receiver",
    );
    assert_eq!(
        caller.call(
            &regexp_search,
            Value::Object(throwing_receiver),
            &[string_value("x")],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller custom exec TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "RegExp Symbol.search replaced a caller-realm user exception",
    );
}
