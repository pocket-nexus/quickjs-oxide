use crate::oracle_cases::GroupedCase as Case;
use crate::runtime_completion_oracle::rust_case_observation as rust_observation;

#[test]
fn json_stringify_keeps_deep_traversal_on_the_engine_owned_stack() {
    // Upstream QuickJS recursively consumes the host C stack here, so its
    // exact failure depth varies with compiler, ABI, and CI thread stack.
    // Keep the cross-engine vector above 256 and retain the 4096-level
    // quickjs-oxide stress as a separate, platform-independent assertion.
    let case = Case {
        group: "deep-iterative-traversal",
        description: "quickjs-oxide traverses 4096 acyclic array levels iteratively",
        source: r#"
            (function () {
                var value = 0;
                for (var index = 0; index < 4096; index++) value = [value];
                return JSON.stringify(value).length;
            })()
        "#,
        expected: "return|number|8193",
    };

    assert_eq!(rust_observation(&case), case.expected);
}
