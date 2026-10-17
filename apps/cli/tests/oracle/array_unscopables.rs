use crate::quickjs_argv_completion_oracle;
use crate::runtime_observation::host_runtime_context;
use crate::runtime_observation::primitive_value_text;
use crate::runtime_oracle::value_type;
use quickjs_argv_completion_oracle::observe_completion_argv_trim_end as observe_oracle;

use quickjs_oxide::engine::api::core::{Context, Runtime};

// This target pins QuickJS 2026-06-04 `js_array_unscopables_funcs` and the
// Symbol.unscopables-specific null-prototype `JS_DEF_OBJECT` autoinit path.

const VALUE_CASES: &[(&str, &str)] = &[
    (
        "repeated reads preserve identity and inner mutations",
        r#"(function(){
            var first=Array.prototype[Symbol.unscopables];
            var second=Array.prototype[Symbol.unscopables];
            first.at=false;first.extra=7;
            return (first===second)+"|"+second.at+"|"+second.extra+"|"+
                (Object.getPrototypeOf(first)===null);
        })()"#,
    ),
    (
        "sloppy assignment cannot replace the outer read-only property",
        r#"(function(){
            var prototype=Array.prototype,original=prototype[Symbol.unscopables];
            prototype[Symbol.unscopables]=Object();
            return prototype[Symbol.unscopables]===original;
        })()"#,
    ),
    (
        "the configurable outer property can be replaced with new attributes",
        r#"(function(){
            var prototype=Array.prototype,replacement=Object(),descriptor=Object();
            descriptor.value=replacement;descriptor.writable=true;descriptor.enumerable=true;
            descriptor.configurable=true;
            Object.defineProperty(prototype,Symbol.unscopables,descriptor);
            prototype[Symbol.unscopables]=9;
            return prototype[Symbol.unscopables]+"|"+(replacement!==prototype[Symbol.unscopables]);
        })()"#,
    ),
    (
        "the outer autoinit property can be deleted before its first Get",
        r#"(function(){
            var prototype=Array.prototype;
            var deleted=delete prototype[Symbol.unscopables];
            return deleted+"|"+(prototype[Symbol.unscopables]===undefined)+"|"+
                (Symbol.unscopables in prototype);
        })()"#,
    ),
    (
        "every inner property is writable enumerable and configurable in behavior",
        r#"(function(){
            var value=Array.prototype[Symbol.unscopables];
            value.at=false;var deleted=delete value.flat;value.added=true;
            return value.at+"|"+deleted+"|"+("flat" in value)+"|"+value.added+"|"+
                Object.prototype.propertyIsEnumerable.call(value,"at")+"|"+
                Object.prototype.propertyIsEnumerable.call(value,"added");
        })()"#,
    ),
    (
        "the pinned table excludes with and includes every completed Array method",
        r#"(function(){
            var value=Array.prototype[Symbol.unscopables];
            return ("with" in value)+"|"+value.at+value.copyWithin+value.entries+value.fill+
                value.find+value.findIndex+value.findLast+value.findLastIndex+value.flat+
                value.flatMap+value.includes+value.keys+value.toReversed+value.toSorted+
                value.toSpliced+value.values;
        })()"#,
    ),
];

#[test]
fn array_unscopables_values_identity_and_mutation_match_pinned_quickjs() {
    let oracle = require_qjs_oracle!("Array unscopables values");
    for &(description, source) in VALUE_CASES {
        let (runtime, mut context) = host_runtime_context();
        assert_eq!(
            observe_rust_eval(&runtime, &mut context, source, description),
            observe_oracle(&oracle, source, description),
            "Array unscopables behavior drifted for {description}",
        );
    }
}

fn observe_rust_eval(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    let value = context
        .eval(source)
        .unwrap_or_else(|error| panic!("Rust rejected {description} ({source:?}): {error}"));
    format!(
        "return|{}|{}",
        value_type(runtime, &value),
        primitive_value_text(value),
    )
}
