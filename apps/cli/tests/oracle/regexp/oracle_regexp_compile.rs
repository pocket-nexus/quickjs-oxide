use crate::runtime_completion_oracle::REGEXP_COMPLETION_PRELUDE as PRELUDE;
use crate::runtime_completion_oracle::compare_read_context_eval_completion_cases_with_prelude;

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
