use crate::object_graph_observation::{define_builtin_data as define_data, intrinsic_prototype};
use crate::runtime_observation::assert_native_error;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::{
    property_callable, take_pending_exception_object as take_exception_object,
};
use quickjs_oxide::engine::api::{JsString, RuntimeError, Value};

// Pins QuickJS 2026-06-04 Annex-B `js_string_CreateHTML` and its selector/tag
// table (quickjs.c 46546-46615), plus the thirteen GenericMagic String
// prototype entries (46661-46674). QuickJS converts the receiver first. The
// four attribute variants then use `JS_ToStringCheckObject` for argv[0], so a
// missing, explicit undefined, or null attribute is rejected. The other nine
// variants ignore every argument. Attribute text replaces only raw U+0022
// with the six ASCII code units of `&quot;`; source text is never escaped.
//
// Every raw String observation is encoded as hexadecimal UTF-16 code units
// before crossing the process boundary, preserving NUL and lone surrogates.

#[test]
fn string_create_html_defining_realms_user_throw_identity_and_caller_construct_error_are_exact() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let anchor = property_callable(&runtime, &mut defining, &defining_prototype, "anchor");
    let big = property_callable(&runtime, &mut defining, &defining_prototype, "big");
    let defining_type_error = intrinsic_prototype(&runtime, &mut defining, "TypeError");
    let caller_type_error = intrinsic_prototype(&runtime, &mut caller, "TypeError");
    assert_ne!(defining_type_error, caller_type_error);
    assert_eq!(
        runtime.get_prototype_of(anchor.as_object()).unwrap(),
        Some(defining.function_prototype().unwrap()),
    );

    assert_native_error(
        &runtime,
        &mut caller,
        &big,
        Value::Null,
        &[],
        &defining_type_error,
    );
    assert_native_error(
        &runtime,
        &mut caller,
        &anchor,
        Value::String(JsString::try_from_utf8("x").unwrap()),
        &[],
        &defining_type_error,
    );
    let attribute_symbol = runtime
        .new_symbol(Some(JsString::try_from_utf8("attribute").unwrap()))
        .unwrap();
    assert_native_error(
        &runtime,
        &mut caller,
        &anchor,
        Value::String(JsString::try_from_utf8("x").unwrap()),
        &[Value::Symbol(attribute_symbol)],
        &defining_type_error,
    );

    let sentinel = caller.new_object().unwrap();
    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "htmlSentinel",
        Value::Object(sentinel.try_clone().expect("duplicate root")),
    );
    let throwing_attribute = caller
        .eval(
            r#"(function(){
                var value=Object();
                value[Symbol.toPrimitive]=function(){throw htmlSentinel};
                return value;
            })()"#,
        )
        .unwrap();
    assert_eq!(
        caller.call(
            &anchor,
            Value::String(JsString::try_from_utf8("x").unwrap()),
            &[throwing_attribute],
        ),
        Err(RuntimeError::Exception),
    );
    assert_eq!(
        caller.take_exception().unwrap(),
        Some(Value::Object(sentinel)),
        "attribute conversion did not preserve the user-thrown value",
    );

    assert_eq!(caller.construct(&anchor, &[]), Err(RuntimeError::Exception));
    assert_eq!(
        runtime
            .get_prototype_of(&take_exception_object(&mut caller))
            .unwrap(),
        Some(caller_type_error),
        "non-constructor rejection did not use the caller realm",
    );
}

#[test]
fn string_create_html_callables_are_per_realm_distinct_and_collectable() {
    let runtime = host_runtime();
    let retained = {
        let mut first = runtime.new_context().expect("create context");
        let mut second = runtime.new_context().expect("create context");
        let first_prototype = first.string_prototype().unwrap();
        let second_prototype = second.string_prototype().unwrap();
        let names = [
            "anchor",
            "big",
            "blink",
            "bold",
            "fixed",
            "fontcolor",
            "fontsize",
            "italics",
            "link",
            "small",
            "strike",
            "sub",
            "sup",
        ];
        let first_functions =
            names.map(|name| property_callable(&runtime, &mut first, &first_prototype, name));
        let second_functions =
            names.map(|name| property_callable(&runtime, &mut second, &second_prototype, name));
        for index in 0..names.len() {
            assert_eq!(
                first_functions[index],
                property_callable(&runtime, &mut first, &first_prototype, names[index]),
            );
            assert_ne!(first_functions[index], second_functions[index]);
            for other in (index + 1)..names.len() {
                assert_ne!(first_functions[index], first_functions[other]);
            }
        }
        assert_eq!(
            runtime
                .get_prototype_of(first_functions[0].as_object())
                .unwrap(),
            Some(first.function_prototype().unwrap()),
        );
        first_functions
    };
    runtime.run_gc().unwrap();
    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    drop(retained);
    runtime.run_gc().unwrap();
    assert_eq!(runtime.heap_counts().expect("runtime state").live, 0);
}

#[test]
fn string_create_html_stack_overflow_uses_the_caller_realm_and_recovers() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = defining.string_prototype().unwrap();
    let bold = property_callable(&runtime, &mut defining, &defining_prototype, "bold");
    let defining_internal_error = intrinsic_prototype(&runtime, &mut defining, "InternalError");
    let caller_internal_error = intrinsic_prototype(&runtime, &mut caller, "InternalError");
    assert_ne!(defining_internal_error, caller_internal_error);

    define_data(
        &runtime,
        &caller.global_object().unwrap(),
        "foreignBold",
        Value::Object(bold.as_object().try_clone().expect("duplicate root")),
    );
    let Value::Object(error) = caller
        .eval(
            r#"(function(){
                var receiver=Object(),localCall=Function.prototype.call;
                function invoke(){return localCall.call(foreignBold,receiver)}
                receiver[Symbol.toPrimitive]=function(){return invoke()};
                try{invoke()}catch(error){return error}
                return Object();
            })()"#,
        )
        .unwrap()
    else {
        panic!("recursive cross-realm CreateHTML did not return an error object");
    };
    assert_eq!(
        runtime.get_prototype_of(&error).unwrap(),
        Some(caller_internal_error),
        "pre-dispatch stack overflow did not use the caller realm",
    );
    assert_eq!(
        caller
            .call(
                &bold,
                Value::String(JsString::try_from_utf8("x").unwrap()),
                &[],
            )
            .unwrap(),
        Value::String(JsString::try_from_utf8("<b>x</b>").unwrap()),
    );
}
