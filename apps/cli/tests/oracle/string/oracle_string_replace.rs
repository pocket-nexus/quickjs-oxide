use crate::runtime_completion_oracle::compare_read_context_eval_completion_cases_with_prelude;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_exception_object;
use crate::runtime_oracle::eval_object;

use crate::object_graph_observation::eval_optional_callable;
use crate::runtime_completion_oracle::REGEXP_REPLACE_PRELUDE as PRELUDE;
use crate::runtime_observation::string_value;
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04 `js_string_replace`
// (`quickjs.c` 45781-45892), including the shared GetSubstitution helper
// (`quickjs.c` 45661-45779), String.prototype.replace, and
// String.prototype.replaceAll.

const RECURSION_CASES: &[(&str, &str)] = &[(
    "String replace protocol recursion overflows catchably and the runtime recovers",
    r#"(function(){
        function recurse(depth,all){
            var search=Object();
            if(all)search[Symbol.match]=false;
            search[Symbol.replace]=function(){
                if(depth!==0)return recurse(depth-1,!all);
                return "done";
            };
            return all?"x".replaceAll(search,"y"):"x".replace(search,"y");
        }
        var finite=recurse(8,false),completion;
        try{recurse(Infinity,false)}
        catch(error){completion=error.name+":"+error.message}
        return finite+"|"+completion+"|"+"abc".replace("b","X")+"|"+
            "aaa".replaceAll("a","X");
    })()"#,
)];

#[test]
fn string_replace_intrinsics_use_their_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let Some(replace) = eval_optional_callable(
        &runtime,
        &mut defining,
        "String.prototype.replace",
        "defining String replace",
    ) else {
        eprintln!("SKIP String replace defining-realm lock: intrinsic is not published yet");
        return;
    };
    let replace_all = eval_optional_callable(
        &runtime,
        &mut defining,
        "String.prototype.replaceAll",
        "defining String replaceAll",
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

    let noncallable = eval_object(
        &mut caller,
        "(function(){var value=Object();value[Symbol.replace]=17;return value})()",
        "caller noncallable replace hook",
    );
    assert_eq!(
        caller.call(
            &replace,
            string_value("abc"),
            &[Value::Object(noncallable), string_value("X")],
        ),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "defining String replace TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error.try_clone().expect("duplicate root")),
        "String replace native TypeError used the caller realm",
    );

    let throwing = eval_object(
        &mut caller,
        r#"(function(){
            var value=Object();
            Object.defineProperty(value,Symbol.replace,{get:function(){
                throw new TypeError("caller");
            }});
            return value;
        })()"#,
        "caller throwing replace hook",
    );
    assert_eq!(
        caller.call(
            &replace,
            string_value("abc"),
            &[Value::Object(throwing), string_value("X")],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller replace getter TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "String replace replaced a caller-realm user exception",
    );

    if let Some(replace_all) = replace_all {
        let foreign_non_global = eval_object(
            &mut caller,
            "/a/",
            "caller non-global RegExp for replaceAll",
        );
        assert_eq!(
            caller.call(
                &replace_all,
                string_value("a"),
                &[Value::Object(foreign_non_global), string_value("X")],
            ),
            Err(RuntimeError::Exception),
        );
        let global_error =
            take_exception_object(&mut caller, "defining String replaceAll global TypeError");
        assert_eq!(
            runtime.get_prototype_of(&global_error).unwrap(),
            Some(defining_type_error),
            "String replaceAll global validation used the caller realm",
        );
    }
}

#[test]
fn string_replace_recursion_matches_pinned_quickjs() {
    std::thread::Builder::new()
        .name("string-replace-oracle-stack".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            compare_read_context_eval_completion_cases_with_prelude(
                PRELUDE,
                "String replace recursion",
                RECURSION_CASES,
            )
        })
        .unwrap()
        .join()
        .unwrap();
}
