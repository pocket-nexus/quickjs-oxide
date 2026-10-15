use crate::runtime_observation::string_value;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_observation::{
    string_property_with_read_context as string_property, take_exception_object,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04 `js_string_match`'s
// Symbol.match branch (`quickjs.c` 45609-45657), abstract RegExpExec
// (`quickjs.c` 48217-48236), and RegExp.prototype[Symbol.match]
// (`quickjs.c` 48252-48333).

#[test]
fn match_intrinsics_use_defining_realms_and_accept_foreign_regexp_brands() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let string_match = eval_callable(
        &runtime,
        &mut defining,
        "String.prototype.match",
        "defining String match",
    );
    let regexp_match = eval_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype[Symbol.match]",
        "defining RegExp Symbol.match",
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
            .eval("RegExp.prototype[Symbol.match]=function(){return 41}")
            .unwrap(),
    );
    drop(
        caller
            .eval("RegExp.prototype[Symbol.match]=function(){return 99}")
            .unwrap(),
    );
    assert_eq!(
        caller
            .call(
                &string_match,
                string_value("subject"),
                &[string_value("needle")],
            )
            .unwrap(),
        Value::Int(41),
        "String match fallback did not use its defining-realm RegExp intrinsic",
    );

    let global_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var count=0,receiver=Object();receiver.flags="g";receiver.lastIndex=8;
            receiver.exec=function(){count++;return count===1?{0:"x"}:null};
            return receiver;
        })()"#,
        "caller global RegExp-like receiver",
    );
    let Value::Object(global_result) = caller
        .call(
            &regexp_match,
            Value::Object(global_receiver),
            &[string_value("x")],
        )
        .unwrap()
    else {
        panic!("global RegExp Symbol.match did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&global_result).unwrap(),
        Some(defining.array_prototype().unwrap()),
        "global match result did not use the method defining realm Array",
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &global_result, "0"),
        "x",
    );

    let foreign_regexp = eval_object(
        &mut caller,
        "(function(){var r=/b/;r.exec=null;return r})()",
        "caller RegExp with builtin exec fallback",
    );
    let Value::Object(foreign_result) = caller
        .call(
            &regexp_match,
            Value::Object(foreign_regexp),
            &[string_value("abc")],
        )
        .unwrap()
    else {
        panic!("foreign branded RegExp did not return its match object");
    };
    assert_eq!(
        runtime.get_prototype_of(&foreign_result).unwrap(),
        Some(defining.array_prototype().unwrap()),
        "foreign branded RegExp match result used the caller Array realm",
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &foreign_result, "0"),
        "b",
    );

    assert_eq!(
        caller.call(&regexp_match, Value::Int(1), &[string_value("x")]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "defining-realm match TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "native RegExp Symbol.match TypeError used the caller realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var receiver=Object();
            Object.defineProperty(receiver,"flags",{get:function(){throw new TypeError("caller flags")}});
            return receiver;
        })()"#,
        "caller throwing flags receiver",
    );
    assert_eq!(
        caller.call(
            &regexp_match,
            Value::Object(throwing_receiver),
            &[string_value("x")],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller flags TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "RegExp Symbol.match replaced a caller-realm user exception",
    );
}

#[test]
fn mixed_string_and_regexp_match_recursion_guard_is_catchable_and_recovers() {
    std::thread::Builder::new()
        .name("string-regexp-match-stack-proof".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let (_runtime, mut context) = host_runtime_context();
            drop(
                context
                    .eval(
                        r#"function mixedMatchRecurse(kind,depth){
                        if(kind===0){
                            var pattern=Object();
                            pattern[Symbol.match]=function(){
                                if(depth!==0)return mixedMatchRecurse(1,depth-1);
                                return null;
                            };
                            return "x".match(pattern);
                        }
                        var regexp=Object();regexp.flags="";
                        regexp.exec=function(){
                            if(depth!==0)mixedMatchRecurse(0,depth-1);
                            return null;
                        };
                        return RegExp.prototype[Symbol.match].call(regexp,"x");
                    }"#,
                    )
                    .unwrap(),
            );

            // The legacy engine uses the original physical native-frame ceiling.
            // Owned callbacks retain the unchanged logical-frame budget instead:
            // a long finite chain completes, while -1 never reaches the base case.

            let (safe_depth, overflow_depth) = (256, -1);

            for (entry, kind) in [("String.prototype.match", 0), ("RegExp @@match", 1)] {
                assert_eq!(
                    context
                        .eval(&format!("mixedMatchRecurse({kind},{safe_depth})"))
                        .unwrap(),
                    Value::Null,
                    "the proven-safe mixed {entry} chain was rejected",
                );
                assert_eq!(
                    context
                        .eval(&format!(
                            r#"(function(){{
                                try{{mixedMatchRecurse({kind},{overflow_depth});return "missing"}}
                                catch(error){{return error.name+":"+error.message}}
                            }})()"#,
                        ))
                        .unwrap(),
                    string_value("InternalError:stack overflow"),
                    "the mixed match recursion limit was not enforced from {entry}",
                );
            }
            assert_eq!(
                context
                    .eval(
                        r#"__arrayForRecovery="abc".match("b");
                           __arrayForRecovery[0]+"|"+
                           RegExp.prototype[Symbol.match].call({
                               flags:"",exec:function(){return null}
                           },"x")"#,
                    )
                    .unwrap(),
                string_value("b|null"),
                "the runtime did not recover after mixed match overflow",
            );
        })
        .expect("2 MiB String/RegExp match stack-proof thread did not start")
        .join()
        .expect("2 MiB String/RegExp match stack-proof thread panicked");
}
