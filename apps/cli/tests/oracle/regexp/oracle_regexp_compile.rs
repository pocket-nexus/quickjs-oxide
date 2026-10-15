use crate::runtime_completion_oracle::REGEXP_COMPLETION_PRELUDE as PRELUDE;
use crate::runtime_completion_oracle::compare_read_context_eval_completion_cases_with_prelude;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::string_value;
use crate::runtime_observation::{
    string_property_with_read_context as string_property, take_exception_object,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{Context, ObjectRef, Runtime, RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04
// `js_regexp_compile` (`quickjs.c` 47584-47626). This Annex B method is
// deliberately tested against the pinned implementation rather than a later
// specification interpretation: QuickJS accepts every genuine RegExp class
// instance, including derived and cross-realm objects.

const RECURSION_CASES: &[(&str, &str)] = &[(
    "RegExp compile conversion recursion overflows catchably and the runtime recovers",
    r#"(function(){
        function recurse(depth){
            var regexp=/a/;
            if(depth===0)return "done";
            regexp.compile({toString:function(){recurse(depth-1);return "a"}});
            return "done";
        }
        var finite=recurse(8),completion;
        try{recurse(Infinity)}
        catch(error){completion=error.name+":"+error.message}
        return finite+"|"+completion+"|"+(/a/.compile("b").test("b"));
    })()"#,
)];

#[test]
fn regexp_compile_intrinsic_uses_defining_realm_and_accepts_foreign_brands() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let compile = eval_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype.compile",
        "defining RegExp compile",
    );
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let defining_syntax_error = eval_object(
        &mut defining,
        "SyntaxError.prototype",
        "defining SyntaxError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    assert_ne!(defining_type_error, caller_type_error);

    let foreign_regexp = eval_object(
        &mut caller,
        "(function(){var r=/old/g;r.lastIndex=7;return r})()",
        "caller RegExp",
    );
    let result = caller
        .call(
            &compile,
            Value::Object(foreign_regexp.try_clone().expect("duplicate root")),
            &[string_value("new")],
        )
        .expect("defining-realm compile on a caller RegExp");
    assert_eq!(
        result,
        Value::Object(foreign_regexp.try_clone().expect("duplicate root")),
        "RegExp compile did not return the foreign receiver",
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &foreign_regexp, "source"),
        "new",
    );
    assert_eq!(
        number_property(&runtime, &mut caller, &foreign_regexp, "lastIndex"),
        0.0,
    );

    let ordinary = eval_object(&mut caller, "({})", "caller ordinary object");
    assert_eq!(
        caller.call(&compile, Value::Object(ordinary), &[]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "defining-realm compile TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "native RegExp compile TypeError used the caller realm",
    );

    let stable = eval_object(
        &mut caller,
        "(function(){var r=/stable/g;r.lastIndex=9;return r})()",
        "caller stable RegExp",
    );
    let throwing_pattern = eval_object(
        &mut caller,
        r#"({toString:function(){throw new TypeError("caller pattern")}})"#,
        "caller throwing pattern",
    );
    assert_eq!(
        caller.call(
            &compile,
            Value::Object(stable.try_clone().expect("duplicate root")),
            &[Value::Object(throwing_pattern)],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller pattern TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "RegExp compile replaced a caller-realm conversion exception",
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &stable, "source"),
        "stable",
    );
    assert_eq!(
        number_property(&runtime, &mut caller, &stable, "lastIndex"),
        9.0,
    );

    assert_eq!(
        caller.call(
            &compile,
            Value::Object(stable.try_clone().expect("duplicate root")),
            &[string_value("[")],
        ),
        Err(RuntimeError::Exception),
    );
    let syntax_error = take_exception_object(&mut caller, "defining-realm compile SyntaxError");
    assert_eq!(
        runtime.get_prototype_of(&syntax_error).unwrap(),
        Some(defining_syntax_error),
        "RegExp compile SyntaxError used the caller realm",
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &stable, "source"),
        "stable",
        "failed compilation mutated the foreign RegExp",
    );
}

#[test]
fn regexp_compile_recursion_is_catchable_and_recovers_like_pinned_quickjs() {
    std::thread::Builder::new()
        .name("regexp-compile-stack-proof".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            compare_read_context_eval_completion_cases_with_prelude(
                PRELUDE,
                "RegExp compile recursion",
                RECURSION_CASES,
            )
        })
        .unwrap()
        .join()
        .unwrap();
}

fn number_property(
    runtime: &Runtime,
    context: &mut Context,
    object: &ObjectRef,
    name: &str,
) -> f64 {
    let key = runtime.intern_property_key(name).unwrap();
    context
        .get_property(object, &key)
        .unwrap_or_else(|error| panic!("read number property {name}: {error}"))
        .as_number()
        .unwrap_or_else(|| panic!("{name} was not a number"))
}
