use crate::runtime_completion_oracle::compare_read_context_eval_completion_cases_with_prelude;

use crate::runtime_completion_oracle::REGEXP_REPLACE_PRELUDE as PRELUDE;

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
