use super::support::*;
use crate::runtime_observation::{host_runtime, host_runtime_context};

use crate::object_graph_observation::int_property;
use quickjs_oxide::engine::api::{RuntimeError, Value};

// This target pins QuickJS 2026-06-04 `JS_FlattenIntoArray` and its shared
// magic-selected `flatMap` / `flat` wrapper.

#[test]
fn array_flatten_basic_rust_smoke() {
    let (_runtime, mut context) = host_runtime_context();
    let value = context
        .eval(r#"[[1,2],[3,[4]],,5].flat().join("|")"#)
        .expect("evaluate basic Array.flat smoke");
    assert_eq!(primitive_value_text(value), "1|2|3|4|5");
    let value = context
        .eval(r#"[1,2,,3].flatMap(function(x,i){return [x,i]}).join("|")"#)
        .expect("evaluate basic Array.flatMap smoke");
    assert_eq!(primitive_value_text(value), "1|0|2|1|3|3");
}

#[test]
fn array_flatten_recursive_mapper_stack_overflow_is_catchable_without_oracle() {
    let (_runtime, mut context) = host_runtime_context();
    // Owned callbacks consume logical frames, so the old native-stack ceiling
    // must not reject this finite chain. Keep the legacy ceiling probe below
    // for the default VM and exercise a true infinite chain for stack-vm.

    assert_eq!(
        primitive_value_text(
            context
                .eval(
                    r#"(function(){
            function nest(count){
                return [1].flatMap(function(value){return count?nest(count-1):[value]})
            }
            return nest(40)[0];
        })()"#
                )
                .unwrap()
        ),
        "1",
    );
    let count = { -1 };
    let value = context
        .eval(&format!(
            r#"(function(){{
                function nest(count){{
                    return [1].flatMap(function(value){{return count?nest(count-1):[value]}})
                }}
                try{{nest({count});return "missing"}}
                catch(error){{return error.name+"|"+error.message}}
            }})()"#,
        ))
        .expect("recursive Array.flatMap completion stays catchable");
    assert_eq!(primitive_value_text(value), "InternalError|stack overflow",);
    assert_eq!(primitive_value_text(context.eval("42").unwrap()), "42");
}

#[test]
fn array_flatten_results_native_errors_and_user_throws_use_pinned_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_array_prototype = defining.array_prototype().unwrap();
    let caller_array_prototype = caller.array_prototype().unwrap();
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
    let flat = property_callable(&runtime, &mut defining, &defining_array_prototype, "flat");
    let flat_map = property_callable(
        &runtime,
        &mut defining,
        &defining_array_prototype,
        "flatMap",
    );

    let receiver = eval_object(&mut caller, "[[10],[20]]", "caller nested Array");
    let Value::Object(result) = caller
        .call(
            &flat,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[],
        )
        .expect("cross-realm Array.flat call")
    else {
        panic!("cross-realm Array.flat result was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "Array.flat result did not use the native defining realm",
    );
    assert_ne!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(caller_array_prototype),
    );
    assert_eq!(int_property(&runtime, &mut caller, &result, "0"), 10);
    assert_eq!(int_property(&runtime, &mut caller, &result, "1"), 20);

    assert!(matches!(
        caller.call(
            &flat_map,
            Value::Object(receiver.try_clone().expect("duplicate root")),
            &[]
        ),
        Err(RuntimeError::Exception),
    ));
    let native_error = take_exception_object(&mut caller, "Array.flatMap mapper TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "Array.flatMap native TypeError did not use the method defining realm",
    );

    let callback = eval_callable(
        &runtime,
        &mut caller,
        "(function(){throw new TypeError('caller mapper')})",
        "caller throwing mapper",
    );
    assert!(matches!(
        caller.call(
            &flat_map,
            Value::Object(receiver),
            &[Value::Object(
                callback.as_object().try_clone().expect("duplicate root")
            )],
        ),
        Err(RuntimeError::Exception),
    ));
    let user_error = take_exception_object(&mut caller, "Array.flatMap user TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "Array.flatMap replaced a user mapper throw with a defining-realm error",
    );
}
