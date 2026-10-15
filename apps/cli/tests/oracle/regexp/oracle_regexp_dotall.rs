use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_exception_object;
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04 RegExp dotAll semantics.
// QuickJS carries `s` through `LRE_FLAG_DOTALL`: `libregexp.c` selects
// REOP_any instead of REOP_dot, scoped modifiers save and restore dotall state,
// and `quickjs.c` exposes the flag through the RegExp accessors and species
// consumers. These vectors focus on interactions not isolated by the broader
// RegExp intrinsic, modifier, matchAll, and split oracle suites.

#[test]
fn regexp_dotall_getter_cross_realm_brand_and_errors_use_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let defining_getter = eval_callable(
        &runtime,
        &mut defining,
        "Object.getOwnPropertyDescriptor(RegExp.prototype,'dotAll').get",
        "defining dotAll getter",
    );
    let caller_getter = eval_callable(
        &runtime,
        &mut caller,
        "Object.getOwnPropertyDescriptor(RegExp.prototype,'dotAll').get",
        "caller dotAll getter",
    );
    let defining_prototype = eval_object(
        &mut defining,
        "RegExp.prototype",
        "defining RegExp prototype",
    );
    let caller_prototype = eval_object(&mut caller, "RegExp.prototype", "caller RegExp prototype");
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
    let caller_dotall = eval_object(&mut caller, "/./s", "caller dotAll RegExp");
    let defining_plain = eval_object(&mut defining, "/./", "defining plain RegExp");

    assert_eq!(
        caller
            .call(&defining_getter, Value::Object(caller_dotall), &[])
            .unwrap(),
        Value::Bool(true),
    );
    assert_eq!(
        defining
            .call(&caller_getter, Value::Object(defining_plain), &[])
            .unwrap(),
        Value::Bool(false),
    );
    assert_eq!(
        defining
            .call(
                &defining_getter,
                Value::Object(defining_prototype.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        Value::Undefined,
    );
    assert_eq!(
        caller
            .call(
                &caller_getter,
                Value::Object(caller_prototype.try_clone().expect("duplicate root")),
                &[],
            )
            .unwrap(),
        Value::Undefined,
    );

    assert_eq!(
        caller.call(&defining_getter, Value::Object(caller_prototype), &[],),
        Err(RuntimeError::Exception),
    );
    let defining_error = take_exception_object(&mut caller, "defining dotAll brand TypeError");
    assert_eq!(
        runtime.get_prototype_of(&defining_error).unwrap(),
        Some(defining_type_error),
    );

    assert_eq!(
        defining.call(&caller_getter, Value::Object(defining_prototype), &[],),
        Err(RuntimeError::Exception),
    );
    let caller_error = take_exception_object(&mut defining, "caller dotAll brand TypeError");
    assert_eq!(
        runtime.get_prototype_of(&caller_error).unwrap(),
        Some(caller_type_error),
    );
}
