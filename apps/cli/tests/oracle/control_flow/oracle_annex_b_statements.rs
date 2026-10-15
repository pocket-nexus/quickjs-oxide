use super::quickjs_argv_completion_oracle::observe_completion_argv_sequence_strip_one_lf as observe_oracle_sequence;
use crate::runtime_completion_oracle::observe_legacy_float_eval_completion as observe_rust_eval;
use crate::runtime_observation::{host_runtime, host_runtime_context};

use quickjs_oxide::engine::api::{
    AccessorValue, DescriptorField, JsString, OrdinaryPropertyDescriptor, Value,
};

#[test]
fn program_label_annex_global_state_matches_pinned_quickjs() {
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
    assert_eq!(
        rust,
        observe_oracle_sequence(&oracle, &sources, description)
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

#[test]
fn program_label_cross_realm_regression() {
    // Pinned with the QuickJS C API compile-in-A/execute-in-B path. The Rust
    // API exposes the same operation directly, so this remains unconditional.
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    drop(defining.eval("globalThis.realmTag='A'").unwrap());
    drop(caller.eval("globalThis.realmTag='B'").unwrap());

    let bytecode = defining
        .compile(
            "label:function crossRealmLabel(){return realmTag+'|'+(this===globalThis)};\
             crossRealmLabel",
        )
        .unwrap();
    let Value::Object(function) = caller.execute(&bytecode).unwrap() else {
        panic!("cross-realm Program label did not return its function");
    };
    let Value::Object(prototype_a) = defining.eval("Function.prototype").unwrap() else {
        panic!("defining Function.prototype was not an object");
    };
    let Value::Object(prototype_b) = caller.eval("Function.prototype").unwrap() else {
        panic!("caller Function.prototype was not an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&function).unwrap(),
        Some(prototype_a)
    );
    assert_ne!(
        runtime.get_prototype_of(&function).unwrap(),
        Some(prototype_b)
    );
    let callable = runtime.as_callable(&function).unwrap().unwrap();
    assert_eq!(
        caller.call(&callable, Value::Undefined, &[]).unwrap(),
        Value::String(JsString::try_from_utf8("B|false").unwrap())
    );
}
