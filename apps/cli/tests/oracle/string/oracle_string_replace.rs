use crate::runtime_completion_oracle::compare_read_context_eval_completion_cases_with_prelude;

use crate::runtime_completion_oracle::REGEXP_REPLACE_PRELUDE as PRELUDE;

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
