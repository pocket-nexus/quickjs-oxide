use crate::runtime_observation::string_value;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_observation::{
    string_property_with_read_context as string_property, take_exception_object,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{RuntimeError, Value};

// Differential lock for pinned QuickJS 2026-06-04
// `js_regexp_Symbol_split` (`quickjs.c` 48875-48990), including
// SpeciesConstructor, abstract RegExpExec, and String.prototype.split's
// Symbol.split activation path.

#[test]
fn regexp_split_intrinsics_use_defining_realms_and_foreign_species() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let regexp_split = eval_callable(
        &runtime,
        &mut defining,
        "RegExp.prototype[Symbol.split]",
        "defining RegExp Symbol.split",
    );
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
    assert_ne!(defining_type_error, caller_type_error);

    let foreign_regexp = eval_object(
        &mut caller,
        "(function(){var r=/b/;r.exec=null;return r})()",
        "caller branded RegExp",
    );
    let Value::Object(foreign_result) = caller
        .call(
            &regexp_split,
            Value::Object(foreign_regexp),
            &[string_value("abc")],
        )
        .expect("cross-realm branded RegExp split")
    else {
        panic!("cross-realm branded RegExp split did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&foreign_result).unwrap(),
        Some(
            defining_array_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "RegExp split result did not use the method defining realm Array",
    );
    assert_ne!(
        runtime.get_prototype_of(&foreign_result).unwrap(),
        Some(caller_array_prototype),
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &foreign_result, "0"),
        "a"
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &foreign_result, "1"),
        "c"
    );

    let foreign_species_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var holder=Object(),regexp=Object();
            holder[Symbol.species]=function(_pattern,flags){
                globalThis.splitSpeciesFlags=flags;
                var splitter=Object(),calls=0;globalThis.splitterPrototype=Object.getPrototypeOf(splitter);
                splitter.lastIndex=0;
                splitter.exec=function(){
                    calls++;globalThis.splitExecCalls=calls;
                    if(calls===2){this.lastIndex=2;return {length:1}}
                    return null;
                };
                return splitter;
            };
            regexp.constructor=holder;regexp.flags="";return regexp;
        })()"#,
        "caller custom RegExp species receiver",
    );
    let Value::Object(species_result) = caller
        .call(
            &regexp_split,
            Value::Object(foreign_species_receiver),
            &[string_value("abc")],
        )
        .expect("cross-realm custom species RegExp split")
    else {
        panic!("cross-realm custom species RegExp split did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&species_result).unwrap(),
        Some(defining_array_prototype),
        "foreign species changed the defining realm of the split result Array",
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &species_result, "0"),
        "a"
    );
    assert_eq!(
        string_property(&runtime, &mut caller, &species_result, "1"),
        "c"
    );
    assert_eq!(caller.eval("splitSpeciesFlags").unwrap(), string_value("y"));
    assert_eq!(caller.eval("splitExecCalls").unwrap(), Value::Int(3));
    assert_eq!(
        caller.eval("splitterPrototype===Object.prototype").unwrap(),
        Value::Bool(true),
        "custom splitter did not retain the species constructor realm",
    );

    assert_eq!(
        caller.call(&regexp_split, Value::Int(1), &[string_value("x")]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "defining-realm split TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "native RegExp Symbol.split TypeError used the caller realm",
    );

    let throwing_receiver = eval_object(
        &mut caller,
        r#"(function(){
            var regexp=Object();
            Object.defineProperty(regexp,"constructor",{get:function(){
                throw new TypeError("caller constructor");
            }});
            return regexp;
        })()"#,
        "caller throwing constructor receiver",
    );
    assert_eq!(
        caller.call(
            &regexp_split,
            Value::Object(throwing_receiver),
            &[string_value("x")],
        ),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller split constructor TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "RegExp Symbol.split replaced a caller-realm user exception",
    );
}

#[test]
fn mixed_string_and_regexp_split_recursion_guard_is_catchable_and_recovers() {
    std::thread::Builder::new()
        .name("string-regexp-split-stack-proof".into())
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let (_runtime, mut context) = host_runtime_context();
            drop(
                context
                    .eval(
                        r#"function mixedSplitRecurse(kind,depth){
                        if(kind===0){
                            var separator=Object();
                            separator[Symbol.split]=function(){
                                if(depth!==0)return mixedSplitRecurse(1,depth-1);
                                return ["leaf"];
                            };
                            return "x".split(separator);
                        }
                        var regexp=Object(),holder=Object();
                        function Species(){
                            var splitter=Object();splitter.lastIndex=0;
                            splitter.exec=function(){
                                if(depth!==0)mixedSplitRecurse(0,depth-1);
                                return null;
                            };
                            return splitter;
                        }
                        holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
                        return RegExp.prototype[Symbol.split].call(regexp,"x");
                    }"#,
                    )
                    .unwrap(),
            );

            // The legacy engine uses the original physical native-frame ceiling.
            // Owned callbacks retain the unchanged logical-frame budget instead:
            // a long finite chain completes, while -1 never reaches the base case.

            let (safe_depth, overflow_depth) = (256, -1);

            for (entry, kind) in [("String.prototype.split", 0), ("RegExp @@split", 1)] {
                assert_eq!(
                    context
                        .eval(&format!("mixedSplitRecurse({kind},{safe_depth}).length"))
                        .unwrap(),
                    Value::Int(1),
                    "the proven-safe mixed {entry} chain was rejected",
                );
                assert_eq!(
                    context
                        .eval(&format!(
                            r#"(function(){{
                                try{{mixedSplitRecurse({kind},{overflow_depth});return "missing"}}
                                catch(error){{return error.name+":"+error.message}}
                            }})()"#,
                        ))
                        .unwrap(),
                    string_value("InternalError:stack overflow"),
                    "the mixed split recursion limit was not enforced from {entry}",
                );
            }
            assert_eq!(
                context
                    .eval(
                        r#"__splitRecovery="a-b".split(/-/);
                           __splitRecovery.join("|")+"|"+
                           (function(){
                               var recovery=Object(),holder=Object();
                               holder[Symbol.species]=function(){
                                   return {lastIndex:0,exec:function(){return null}};
                               };
                               recovery.constructor=holder;recovery.flags="";
                               return RegExp.prototype[Symbol.split].call(recovery,"x").join("|");
                           })()"#,
                    )
                    .unwrap(),
                string_value("a|b|x"),
                "the runtime did not recover after mixed split overflow",
            );
        })
        .expect("2 MiB String/RegExp split stack-proof thread did not start")
        .join()
        .expect("2 MiB String/RegExp split stack-proof thread panicked");
}
