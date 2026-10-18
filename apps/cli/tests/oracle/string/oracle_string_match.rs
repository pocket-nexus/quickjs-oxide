use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::string_value;
use quickjs_oxide::engine::api::core::{Value, declare_thread_stack};

// Differential lock for pinned QuickJS 2026-06-04 `js_string_match`'s
// Symbol.match branch (`quickjs.c` 45609-45657), abstract RegExpExec
// (`quickjs.c` 48217-48236), and RegExp.prototype[Symbol.match]
// (`quickjs.c` 48252-48333).

#[test]
fn mixed_string_and_regexp_match_recursion_guard_is_catchable_and_recovers() {
    // A declared stack sizes the native re-entry budget; a debug build's
    // frames need more of it than QuickJS's C frames.
    const STACK: usize = 32 * 1024 * 1024;
    std::thread::Builder::new()
        .name("string-regexp-match-stack-proof".into())
        .stack_size(STACK)
        .spawn(|| {
            declare_thread_stack(STACK);
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

            // A long finite chain completes, while -1 never reaches the base
            // case.

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
        .expect("String/RegExp match stack-proof thread did not start")
        .join()
        .expect("String/RegExp match stack-proof thread panicked");
}
