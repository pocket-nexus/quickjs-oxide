use crate::runtime_completion_oracle::compare_read_context_eval_completion_cases_with_prelude;
use crate::runtime_observation::host_runtime;
use crate::runtime_observation::take_exception_object;
use crate::runtime_oracle::eval_object;

use crate::object_graph_observation::eval_optional_callable;
use crate::runtime_completion_oracle::REGEXP_REPLACE_PRELUDE as PRELUDE;
use crate::runtime_observation::string_value;
use quickjs_oxide::engine::api::{
    AccessorValue, DescriptorField, OrdinaryPropertyDescriptor, RuntimeError, Value,
};

// Differential lock for pinned QuickJS 2026-06-04
// `js_regexp_Symbol_replace` (`quickjs.c` 48622-48815), its direct matcher
// fast path (`quickjs.c` 48101-48215), abstract RegExpExec
// (`quickjs.c` 48217-48236), and shared GetSubstitution
// (`quickjs.c` 45661-45779).

const RECURSION_CASES: &[(&str, &str)] = &[(
    "RegExp Symbol.replace exec recursion overflows catchably and the runtime recovers",
    r#"(function(){
        function recurse(depth){
            var receiver=Object();receiver.flags="";
            receiver.exec=function(){
                if(depth!==0)recurse(depth-1);
                return null;
            };
            return RegExp.prototype[Symbol.replace].call(receiver,"x","y");
        }
        var finite=recurse(8),completion;
        try{recurse(Infinity)}
        catch(error){completion=error.name+":"+error.message}
        return finite+"|"+completion+"|"+/b/[Symbol.replace]("abc","X")+"|"+
            /a/g[Symbol.replace]("aaa","X");
    })()"#,
)];

#[test]
fn regexp_replace_standard_fast_path_accepts_cross_realm_native_targets() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let exec = eval_optional_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype.exec",
        "foreign RegExp exec",
    )
    .expect("foreign RegExp exec was not callable");
    let flags = eval_optional_callable(
        &runtime,
        &mut defining,
        "Object.getOwnPropertyDescriptor(RegExp.prototype,'flags').get",
        "foreign RegExp flags getter",
    )
    .expect("foreign RegExp flags getter was not callable");
    let global = eval_optional_callable(
        &runtime,
        &mut defining,
        "Object.getOwnPropertyDescriptor(RegExp.prototype,'global').get",
        "foreign RegExp global getter",
    )
    .expect("foreign RegExp global getter was not callable");
    let unicode = eval_optional_callable(
        &runtime,
        &mut defining,
        "Object.getOwnPropertyDescriptor(RegExp.prototype,'unicode').get",
        "foreign RegExp unicode getter",
    )
    .expect("foreign RegExp unicode getter was not callable");
    let prototype = eval_object(&mut caller, "RegExp.prototype", "caller RegExp prototype");

    let exec_key = runtime.intern_property_key("exec").unwrap();
    let exec_object = exec.as_object().try_clone().expect("duplicate root");
    assert!(
        runtime
            .define_own_property(
                &prototype,
                &exec_key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Value::Object(exec_object)),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap(),
        "foreign RegExp exec definition was rejected",
    );
    for (name, getter) in [("flags", flags), ("global", global), ("unicode", unicode)] {
        let key = runtime.intern_property_key(name).unwrap();
        assert!(
            runtime
                .define_own_property(
                    &prototype,
                    &key,
                    &OrdinaryPropertyDescriptor {
                        get: DescriptorField::Present(AccessorValue::Callable(getter)),
                        set: DescriptorField::Present(AccessorValue::Undefined),
                        enumerable: DescriptorField::Present(false),
                        configurable: DescriptorField::Present(true),
                        ..OrdinaryPropertyDescriptor::new()
                    },
                )
                .unwrap(),
            "foreign RegExp {name} getter definition was rejected",
        );
    }

    let Value::String(result) = caller
        .eval(
            r#"(function(){
                var hits=0,regexp=/a/;
                Object.defineProperty(regexp,"ignoreCase",{get:function(){
                    hits++;return false;
                }});
                return regexp[Symbol.replace]("a","X")+"|"+hits;
            })()"#,
        )
        .unwrap()
    else {
        panic!("cross-realm standard RegExp replacement did not return a string");
    };
    assert_eq!(result.to_utf8_lossy(), "X|0");
}

#[test]
fn regexp_replace_intrinsic_uses_its_defining_realm() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let Some(replace) = eval_optional_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype[Symbol.replace]",
        "defining RegExp Symbol.replace",
    ) else {
        eprintln!("SKIP RegExp replace defining-realm lock: intrinsic is not published yet");
        return;
    };

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

    assert_eq!(
        caller.call(
            &replace,
            Value::Int(1),
            &[string_value("x"), string_value("y")]
        ),
        Err(RuntimeError::Exception),
    );
    let native_error =
        take_exception_object(&mut caller, "defining RegExp Symbol.replace TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "RegExp Symbol.replace native TypeError used the caller realm",
    );

    let foreign = eval_object(
        &mut caller,
        r#"(function(){
            var value=Object();value.flags="";
            value.exec=function(){return null};
            return value;
        })()"#,
        "caller abstract RegExp receiver",
    );
    assert_eq!(
        caller
            .call(
                &replace,
                Value::Object(foreign),
                &[string_value("subject"), string_value("X")],
            )
            .unwrap(),
        string_value("subject"),
        "defining RegExp Symbol.replace rejected a foreign ordinary receiver",
    );

    let throwing = eval_object(
        &mut caller,
        r#"(function(){
            var value=Object();value.flags="";
            Object.defineProperty(value,"exec",{get:function(){
                throw new TypeError("caller");
            }});
            return value;
        })()"#,
        "caller throwing exec getter",
    );
    assert_eq!(
        caller.call(
            &replace,
            Value::Object(throwing),
            &[string_value("subject"), string_value("X")],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller exec getter TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "RegExp Symbol.replace replaced a caller-realm user exception",
    );
}

#[test]
fn regexp_replace_recursion_matches_pinned_quickjs() {
    std::thread::Builder::new()
        .name("regexp-replace-oracle-stack".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            compare_read_context_eval_completion_cases_with_prelude(
                PRELUDE,
                "RegExp replace recursion",
                RECURSION_CASES,
            )
        })
        .unwrap()
        .join()
        .unwrap();
}
