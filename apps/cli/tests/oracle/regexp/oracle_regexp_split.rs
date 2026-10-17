use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::string_value;
use quickjs_oxide::engine::api::core::Value;

// Differential lock for pinned QuickJS 2026-06-04
// `js_regexp_Symbol_split` (`quickjs.c` 48875-48990), including
// SpeciesConstructor, abstract RegExpExec, and String.prototype.split's
// Symbol.split activation path.

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
