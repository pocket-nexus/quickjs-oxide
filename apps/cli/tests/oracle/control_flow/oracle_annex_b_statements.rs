use super::quickjs_argv_completion_oracle::observe_completion_argv_sequence_strip_one_lf as observe_oracle_sequence;
use crate::runtime_completion_oracle::observe_legacy_float_eval_completion as observe_rust_eval;
use crate::runtime_observation::host_runtime_context;

use quickjs_oxide::engine::api::core::{
    AccessorValue, DescriptorField, OrdinaryPropertyDescriptor, Value,
};

/// QuickJS runs the first script and leaves a masked global lexical, which
/// quickjs-oxide keeps (the specification makes the script an early
/// SyntaxError: the labelled function is var-declared beside `let`, recorded
/// in the M4b Annex B backlog). The last read follows the global
/// Environment Record: an uninitialized lexical binding is a ReferenceError
/// for every reference (QUICKJS-SPEC-025), where QuickJS reads the global
/// object.
#[test]
fn program_label_annex_global_state() {
    let oracle = require_qjs_oracle!("Program label global-state differential");
    let description = "Program labelled Annex precedes a later global lexical";
    let sources = [
        "label:function orderedLabelCollision(){};let orderedLabelCollision;",
        "typeof globalThis.orderedLabelCollision",
        "orderedLabelCollision",
    ];
    let (runtime, mut context) = host_runtime_context();
    let rust = sources
        .iter()
        .map(|source| observe_rust_eval(&runtime, &mut context, source, description))
        .collect::<Vec<_>>()
        .join("\n");
    let quickjs = observe_oracle_sequence(&oracle, &sources, description);
    let (rust_last, quickjs_last) = (rust.lines().last(), quickjs.lines().last());
    assert_eq!(
        rust.lines().take(2).collect::<Vec<_>>(),
        quickjs.lines().take(2).collect::<Vec<_>>()
    );
    assert_eq!(
        rust_last,
        Some("throw|object|ReferenceError|orderedLabelCollision is not initialized")
    );
    assert_ne!(
        rust_last, quickjs_last,
        "QUICKJS-SPEC-025 no longer differs"
    );
}

#[test]
fn program_label_invokes_existing_global_setter_twice() {
    let oracle = require_qjs_oracle!("Program label accessor differential");
    let description = "Program labelled function performs both QuickJS global writes";
    let sources = [
        "Function.labelSetterHits=0;Function.labelSetterValue=0;Object.defineProperty(globalThis,'__qjo_label_accessor',{configurable:true,get:function(){return Function.labelSetterValue},set:function(value){Function.labelSetterHits++;Function.labelSetterValue=value}});0",
        "label:function __qjo_label_accessor(){return 17};Function.labelSetterHits+'|'+typeof __qjo_label_accessor+'|'+__qjo_label_accessor()",
    ];
    let (runtime, mut context) = host_runtime_context();
    drop(
        context
            .eval("Function.labelSetterHits=0;Function.labelSetterValue=0")
            .unwrap(),
    );
    let Value::Object(getter) = context
        .eval("(function(){return Function.labelSetterValue})")
        .unwrap()
    else {
        panic!("Program label accessor getter was not a function");
    };
    let Value::Object(setter) = context
        .eval("(function(value){Function.labelSetterHits++;Function.labelSetterValue=value})")
        .unwrap()
    else {
        panic!("Program label accessor setter was not a function");
    };
    let getter = runtime.as_callable(&getter).unwrap().unwrap();
    let setter = runtime.as_callable(&setter).unwrap().unwrap();
    let global = context.global_object().unwrap();
    let key = runtime.intern_property_key("__qjo_label_accessor").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter)),
                    set: DescriptorField::Present(AccessorValue::Callable(setter)),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    let rust = format!(
        "return|number|0\n{}",
        observe_rust_eval(&runtime, &mut context, sources[1], description)
    );
    assert_eq!(
        rust,
        observe_oracle_sequence(&oracle, &sources, description)
    );
}
