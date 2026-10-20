use super::*;
use crate::engine::test_support::assert_script_true;
use crate::engine::test_support::test_context;

#[test]
fn program_lexicals_lower_to_source_ordered_global_declarations() {
    let source = "let first=1,second=function(){return later};const later=2;first+second()";
    let script = compile_unlinked_script(source).unwrap();
    assert_eq!(script.local_definitions().len(), 1);
    assert_eq!(script.closure_variables().len(), 3);

    let declaration_names = script
        .closure_variables()
        .iter()
        .map(|descriptor| {
            assert_eq!(descriptor.source, ClosureSource::GlobalDeclaration);
            assert!(descriptor.is_lexical);
            let ClosureVariableName::Constant(index) = descriptor.name else {
                panic!("global declaration lost its semantic name");
            };
            let crate::engine::value::PrimitiveValue::String(name) = script.constants()
                [index as usize]
                .as_primitive()
                .expect("global declaration name is not primitive")
            else {
                panic!("global declaration name is not a string");
            };
            name.to_utf8_lossy()
        })
        .collect::<Vec<_>>();
    assert_eq!(declaration_names, ["first", "second", "later"]);
    assert_eq!(
        script
            .code()
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::PutVarInit(_)))
            .count(),
        3
    );

    let child = script
        .constants()
        .iter()
        .find_map(|constant| constant.as_child())
        .expect("global lexical initializer lost its child function");
    assert_eq!(child.closure_variables().len(), 1);
    assert_eq!(
        child.closure_variables()[0].source,
        ClosureSource::ParentGlobal(2)
    );
    assert!(child.closure_variables()[0].is_lexical);
    assert!(child.closure_variables()[0].is_const);

    let stripped =
        compile_unlinked_script_with_filename(source, "<global-strip>", DebugInfoMode::StripDebug)
            .unwrap();
    assert!(stripped.closure_variables().iter().all(|descriptor| {
        descriptor.source == ClosureSource::GlobalDeclaration
            && matches!(descriptor.name, ClosureVariableName::Constant(_))
    }));
    let stripped_child = stripped
        .constants()
        .iter()
        .find_map(|constant| constant.as_child())
        .expect("stripped global lexical lost its child function");
    assert!(matches!(
        stripped_child.closure_variables()[0].name,
        ClosureVariableName::Constant(_)
    ));
}

#[test]
fn program_vars_keep_every_source_ordered_global_declaration() {
    let source = "var first;{var first;var second=function(){return later}}if(false)var later=3;for(var loop=0;false;){}";
    let script = compile_unlinked_script(source).unwrap();
    assert_eq!(script.local_definitions().len(), 1);

    let declaration_names = script
        .closure_variables()
        .iter()
        .map(|descriptor| {
            assert_eq!(descriptor.source, ClosureSource::GlobalDeclaration);
            assert!(!descriptor.is_lexical);
            assert!(!descriptor.is_const);
            assert_eq!(descriptor.kind, ClosureVariableKind::Normal);
            let ClosureVariableName::Constant(index) = descriptor.name else {
                panic!("global var declaration lost its semantic name");
            };
            let crate::engine::value::PrimitiveValue::String(name) = script.constants()
                [index as usize]
                .as_primitive()
                .expect("global var declaration name is not primitive")
            else {
                panic!("global var declaration name is not a string");
            };
            name.to_utf8_lossy()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        declaration_names,
        ["first", "first", "second", "later", "loop"]
    );
    assert_eq!(
        script
            .code()
            .iter()
            .filter(|instruction| matches!(instruction, Instruction::PutVar(_)))
            .count(),
        3
    );
    assert!(
        !script
            .code()
            .iter()
            .any(|instruction| matches!(instruction, Instruction::PutVarInit(_)))
    );

    let child = script
        .constants()
        .iter()
        .find_map(|constant| constant.as_child())
        .expect("global var initializer lost its child function");
    assert_eq!(child.closure_variables().len(), 1);
    assert_eq!(
        child.closure_variables()[0].source,
        ClosureSource::ParentGlobal(3)
    );
    assert!(!child.closure_variables()[0].is_lexical);

    let stripped = compile_unlinked_script_with_filename(
        source,
        "<global-var-strip>",
        DebugInfoMode::StripDebug,
    )
    .unwrap();
    assert!(stripped.closure_variables().iter().all(|descriptor| {
        descriptor.source == ClosureSource::GlobalDeclaration
            && matches!(descriptor.name, ClosureVariableName::Constant(_))
    }));
}

#[test]
fn program_functions_keep_descriptors_but_hoist_into_the_first_name_slot() {
    let script = compile_unlinked_script(
        "let mixed;function mixed(){return mixed}function repeated(){return 1}function repeated(){return 2}repeated",
    )
    .unwrap();
    assert_eq!(script.closure_variables().len(), 4);
    assert!(script.closure_variables()[0].is_lexical);
    assert_eq!(
        script.closure_variables()[1].kind,
        ClosureVariableKind::GlobalFunction
    );
    assert_eq!(
        script.closure_variables()[2].kind,
        ClosureVariableKind::GlobalFunction
    );
    assert_eq!(
        script.closure_variables()[3].kind,
        ClosureVariableKind::GlobalFunction
    );
    assert!(matches!(script.code()[0], Instruction::FClosure(0)));
    assert!(matches!(script.code()[1], Instruction::PutVarInit(0)));
    assert!(matches!(script.code()[2], Instruction::FClosure(1)));
    assert!(matches!(script.code()[3], Instruction::PutVarInit(2)));
    assert!(matches!(script.code()[4], Instruction::FClosure(2)));
    assert!(matches!(script.code()[5], Instruction::PutVarInit(2)));

    let children = script
        .constants()
        .iter()
        .filter_map(|constant| constant.as_child())
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 3);
    assert_eq!(children[0].metadata().function_name_local, None);
    assert_eq!(
        children[0].closure_variables()[0].source,
        ClosureSource::ParentGlobal(0)
    );
    assert!(children[0].closure_variables()[0].is_lexical);
    for child in &children[1..] {
        assert_eq!(child.metadata().function_name_local, None);
    }
}

#[test]
fn program_function_then_lexical_remains_a_source_ordered_syntax_error() {
    let error = compile_unlinked_script("function clash(){};let clash").unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Syntax);
    assert_eq!(error.message(), "invalid redefinition of global identifier");
    assert!(compile_unlinked_script("let clash;function clash(){}").is_ok());
}

#[test]
fn program_vars_instantiate_persist_and_preserve_existing_properties() {
    let (runtime, mut context) = test_context();
    assert_eq!(
        context
            .eval("var value;var value=2;if(false){var dormant=3}for(var loop=0;loop<2;loop++){};value+'|'+typeof dormant+'|'+loop")
            .unwrap(),
        Value::String(JsString::from_static("2|undefined|2"))
    );
    let global = context.global_object().unwrap();
    for (name, value) in [
        ("value", Value::Int(2)),
        ("dormant", Value::Undefined),
        ("loop", Value::Int(2)),
    ] {
        let key = runtime.intern_property_key(name).unwrap();
        assert_eq!(
            context.get_own_property(&global, &key).unwrap(),
            Some(CompleteOrdinaryPropertyDescriptor::Data {
                value,
                writable: true,
                enumerable: true,
                configurable: false,
            })
        );
    }
    assert_eq!(context.eval("var value;value").unwrap(), Value::Int(2));
    assert_eq!(
        context
            .eval("var captured=1,read=function(){return captured};captured=4;read()")
            .unwrap(),
        Value::Int(4)
    );
    assert_eq!(context.eval("delete value").unwrap(), Value::Bool(false));

    assert_eq!(context.eval("hostValue=7").unwrap(), Value::Int(7));
    let host = runtime.intern_property_key("hostValue").unwrap();
    assert_eq!(
        context.eval("var hostValue;hostValue").unwrap(),
        Value::Int(7)
    );
    assert_eq!(
        context.get_own_property(&global, &host).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Int(7),
            writable: true,
            enumerable: true,
            configurable: true,
        })
    );
    assert_script_true(
        &mut context,
        "Function.hostRead=function(){return hostValue};var hostValue=8;delete hostValue",
    );
    assert!(matches!(
        context.eval("Function.hostRead()"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(
        context.eval("var hostValue;hostValue").unwrap(),
        Value::Undefined
    );
    assert_eq!(
        context.eval("Function.hostRead()").unwrap(),
        Value::Undefined
    );
    assert_eq!(
        context.get_own_property(&global, &host).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: false,
        })
    );

    let fixed = runtime.intern_property_key("fixedVar").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &fixed,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Value::Int(1)),
                    writable: DescriptorField::Present(false),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        context.eval("var fixedVar;fixedVar").unwrap(),
        Value::Int(1)
    );
    assert_eq!(
        context.eval("var fixedVar=2;fixedVar").unwrap(),
        Value::Int(1)
    );
    assert!(matches!(
        context.eval("'use strict';var fixedVar=2"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(
        context.get_own_property(&global, &fixed).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Int(1),
            writable: false,
            enumerable: false,
            configurable: true,
        })
    );

    assert_eq!(
        context.eval("Function.varSetterHits=0").unwrap(),
        Value::Int(0)
    );
    let Value::Object(setter) = context
        .eval("(function(value){Function.varSetterHits=value})")
        .unwrap()
    else {
        panic!("global var accessor probe did not create a setter");
    };
    let setter = runtime.as_callable(&setter).unwrap().unwrap();
    let accessor = runtime.intern_property_key("accessorVar").unwrap();
    assert!(
        context
            .define_own_property(
                &global,
                &accessor,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Undefined),
                    set: DescriptorField::Present(AccessorValue::Callable(
                        setter.try_clone().expect("duplicate root")
                    )),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        context
            .eval("var accessorVar;Function.varSetterHits")
            .unwrap(),
        Value::Int(0),
        "a var without initializer must not invoke an existing setter"
    );
    assert_eq!(
        context
            .eval("var accessorVar=9;Function.varSetterHits")
            .unwrap(),
        Value::Int(9)
    );
    assert_eq!(
        context.get_own_property(&global, &accessor).unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Accessor {
            get: None,
            set: Some(setter),
            enumerable: false,
            configurable: true,
        })
    );

    let inherited = runtime.new_context().expect("create context");
    let inherited_key = runtime.intern_property_key("inheritedVar").unwrap();
    let prototype = inherited.object_prototype().unwrap();
    assert!(
        inherited
            .define_own_property(
                &prototype,
                &inherited_key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Value::Int(5)),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(true),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        inherited.eval("var inheritedVar;inheritedVar").unwrap(),
        Value::Undefined
    );
    let inherited_global = inherited.global_object().unwrap();
    assert_eq!(
        inherited
            .get_own_property(&inherited_global, &inherited_key)
            .unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: false,
        })
    );

    let auto_init = runtime.new_context().expect("create context");
    assert_eq!(
        auto_init.eval("var Number;typeof Number").unwrap(),
        Value::String(JsString::from_static("function"))
    );
    assert_eq!(
        auto_init.eval("var Number=9;Number").unwrap(),
        Value::Int(9)
    );
    let number = runtime.intern_property_key("Number").unwrap();
    let auto_init_global = auto_init.global_object().unwrap();
    assert_eq!(
        auto_init
            .get_own_property(&auto_init_global, &number)
            .unwrap(),
        Some(CompleteOrdinaryPropertyDescriptor::Data {
            value: Value::Int(9),
            writable: true,
            enumerable: false,
            configurable: true,
        })
    );
}

#[test]
fn program_var_preflight_conflicts_and_parser_scope() {
    let (runtime, context) = test_context();
    assert_eq!(
        context.eval("let existingLexical=1").unwrap(),
        Value::Undefined
    );
    let Err(RuntimeError::Exception(Value::Object(error))) = context.eval("var existingLexical")
    else {
        panic!("global var/lexical conflict did not throw an Error object");
    };
    let name = runtime.intern_property_key("name").unwrap();
    let message = runtime.intern_property_key("message").unwrap();
    assert_eq!(
        context.get_property(&error, &name).unwrap(),
        Value::String(JsString::from_static("SyntaxError"))
    );
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("redeclaration of 'existingLexical'"))
    );
    assert_eq!(
        context.eval("globalThis.varMarker=0").unwrap(),
        Value::Int(0)
    );
    assert!(matches!(
        context.eval("varMarker=1;var freshBefore=(varMarker=2),existingLexical=(varMarker=3),freshAfter=(varMarker=4)"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(context.eval("varMarker").unwrap(), Value::Int(0));
    assert_eq!(
        context
            .eval("typeof freshBefore+'|'+typeof freshAfter")
            .unwrap(),
        Value::String(JsString::from_static("undefined|undefined"))
    );

    let sealed_runtime = Runtime::new();
    let sealed = sealed_runtime.new_context().expect("create context");
    assert_eq!(
        sealed.eval("let sealedLexical=1").unwrap(),
        Value::Undefined
    );
    let sealed_global = sealed.global_object().unwrap();
    sealed_runtime.prevent_extensions(&sealed_global).unwrap();
    let Err(RuntimeError::Exception(Value::Object(error))) = sealed.eval("var sealedLexical")
    else {
        panic!("sealed global var declaration did not throw an Error object");
    };
    // 16.1.7 step 4: a var name with a lexical declaration is a
    // SyntaxError before the global object's definability is checked.
    assert_eq!(
        sealed.get_property(&error, &name).unwrap(),
        Value::String(JsString::from_static("SyntaxError"))
    );
    assert_eq!(
        sealed.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("redeclaration of 'sealedLexical'"))
    );

    let atomic_runtime = Runtime::new();
    let atomic = atomic_runtime.new_context().expect("create context");
    assert_eq!(
        atomic
            .eval("globalThis.atomicExisting=5;globalThis.atomicMarker=0")
            .unwrap(),
        Value::Int(0)
    );
    let atomic_global = atomic.global_object().unwrap();
    atomic_runtime.prevent_extensions(&atomic_global).unwrap();
    assert!(matches!(
        atomic.eval("atomicMarker=1;var atomicExisting=6,atomicMissing=7"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(atomic.eval("atomicMarker").unwrap(), Value::Int(0));
    assert_eq!(atomic.eval("atomicExisting").unwrap(), Value::Int(5));
    assert_eq!(
        atomic.eval("typeof atomicMissing").unwrap(),
        Value::String(JsString::from_static("undefined"))
    );

    for (source, expected) in [
        (
            "let conflict;var conflict",
            "invalid redefinition of lexical identifier",
        ),
        (
            "var conflict;let conflict",
            "invalid redefinition of global identifier",
        ),
        (
            "{var conflict;var conflict;let conflict}",
            "invalid redefinition of global identifier",
        ),
        (
            "{let conflict;var conflict}",
            "invalid redefinition of lexical identifier",
        ),
    ] {
        assert_eq!(
            compile_unlinked_script(source).unwrap_err().message(),
            expected,
            "{source}"
        );
    }
    for source in [
        "var allowed;{var allowed;let allowed}",
        "{var sibling}{var sibling;let sibling}",
        "{let shadow}var shadow",
    ] {
        compile_unlinked_script(source).unwrap();
    }
}

#[test]
fn program_global_lexical_preflight_and_failed_initializers() {
    let (runtime, context) = test_context();
    let name = runtime.intern_property_key("name").unwrap();
    let message = runtime.intern_property_key("message").unwrap();

    let delayed = context
        .compile("let delayedGlobal=7;delayedGlobal")
        .unwrap();
    assert_eq!(
        context.eval("typeof delayedGlobal").unwrap(),
        Value::String(JsString::from_static("undefined"))
    );
    assert_eq!(context.execute(&delayed).unwrap(), Value::Int(7));
    assert!(matches!(
        context.execute(&delayed),
        Err(RuntimeError::Exception(_))
    ));

    let atomic = context
        .compile("let untouched=function(){return Infinity},NaN=1,Infinity=2")
        .unwrap();
    let Err(RuntimeError::Exception(Value::Object(error))) = context.execute(&atomic) else {
        panic!("global declaration preflight did not throw an Error object");
    };
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("redeclaration of 'NaN'"))
    );
    assert_eq!(
        context.eval("typeof untouched").unwrap(),
        Value::String(JsString::from_static("undefined"))
    );
    assert_eq!(
        context.eval("let untouched=4;untouched").unwrap(),
        Value::Int(4)
    );

    assert_eq!(
        context
            .eval(
                "Function.saved=function(){return captured};let captured=(function(){throw 17})()"
            )
            .err(),
        Some(RuntimeError::Exception(Value::Int(17)))
    );
    let Err(RuntimeError::Exception(Value::Object(error))) = context.eval("Function.saved()")
    else {
        panic!("declaring-script capture did not preserve the global lexical TDZ");
    };
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("captured is not initialized"))
    );
    let Err(RuntimeError::Exception(exception)) = context.eval("captured") else {
        panic!("expected a JavaScript exception");
    };
    // The binding stays uninitialized: every reference to it is in its TDZ
    // (9.1.1.4.6 GetBindingValue), `typeof` too.
    let Value::Object(error) = exception else {
        panic!("later eval did not throw the global lexical's TDZ error");
    };
    assert_eq!(
        context.get_property(&error, &name).unwrap(),
        Value::String(JsString::from_static("ReferenceError"))
    );
    assert_eq!(
        context.get_property(&error, &message).unwrap(),
        Value::String(JsString::from_static("captured is not initialized"))
    );
    assert!(matches!(
        context.eval("typeof captured"),
        Err(RuntimeError::Exception(_))
    ));
    assert_eq!(context.eval("delete captured").unwrap(), Value::Bool(false));
    assert!(matches!(
        context.eval("let captured=1"),
        Err(RuntimeError::Exception(_))
    ));
}
