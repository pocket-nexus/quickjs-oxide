use crate::runtime_observation::host_runtime;
use quickjs_oxide::engine::api::{
    AccessorValue, CallableRef, Context, DescriptorField, ObjectRef, OrdinaryPropertyDescriptor,
    PropertyKey, Runtime, RuntimeError, Value,
};

#[test]
fn captured_global_this_tracks_the_defining_realm_across_property_transitions() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_global = defining.global_object().unwrap();
    let caller_global = caller.global_object().unwrap();
    let key = runtime.intern_property_key("globalThis").unwrap();

    assert_ne!(defining_global, caller_global);
    assert_eq!(
        defining.eval("globalThis").unwrap(),
        Value::Object(defining_global.try_clone().expect("duplicate root"))
    );
    assert_eq!(
        caller.eval("globalThis").unwrap(),
        Value::Object(caller_global.try_clone().expect("duplicate root"))
    );

    let reader = function(
        &runtime,
        &mut defining,
        "(function() { return globalThis; })",
    );
    assert_eq!(
        caller.call(&reader, Value::Undefined, &[]).unwrap(),
        Value::Object(defining_global.try_clone().expect("duplicate root")),
        "a foreign call must resolve globalThis in the bytecode's defining realm"
    );

    let getter = function(
        &runtime,
        &mut defining,
        "(function() { \"use strict\"; return this; })",
    );
    assert!(
        defining
            .define_own_property(
                &defining_global,
                &key,
                &OrdinaryPropertyDescriptor {
                    get: DescriptorField::Present(AccessorValue::Callable(getter)),
                    set: DescriptorField::Present(AccessorValue::Undefined),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
    assert_eq!(
        caller.call(&reader, Value::Undefined, &[]).unwrap(),
        Value::Object(defining_global.try_clone().expect("duplicate root")),
        "the captured global VarRef must fall through to the replacement accessor"
    );

    assert!(runtime.delete_property(&defining_global, &key).unwrap());
    assert!(matches!(
        caller.call(&reader, Value::Undefined, &[]),
        Err(RuntimeError::Exception)
    ));
    let Value::Object(exception) = caller.take_exception().unwrap().unwrap() else {
        panic!("missing captured globalThis did not throw an Error object");
    };
    let defining_reference_error = reference_error_prototype(&runtime, &mut defining);
    let caller_reference_error = reference_error_prototype(&runtime, &mut caller);
    assert_ne!(defining_reference_error, caller_reference_error);
    assert_eq!(
        runtime.get_prototype_of(&exception).unwrap(),
        Some(defining_reference_error),
        "the missing-name ReferenceError must be allocated in the reader's defining realm"
    );

    define_global_this(&mut defining, &defining_global, &key);
    assert_eq!(
        caller.call(&reader, Value::Undefined, &[]).unwrap(),
        Value::Object(defining_global),
        "the captured global VarRef must reconnect to a later data property"
    );
    assert_eq!(
        caller.eval("globalThis").unwrap(),
        Value::Object(caller_global),
        "mutating the defining realm must not replace the caller realm's self reference"
    );
}

#[test]
fn global_this_var_ref_cycle_is_collectable_after_context_drop() {
    let runtime = host_runtime();
    {
        let mut context = runtime.new_context().expect("create context");
        let global = context.global_object().unwrap();
        let reader = function(
            &runtime,
            &mut context,
            "(function() { return globalThis; })",
        );
        assert_eq!(
            context.call(&reader, Value::Undefined, &[]).unwrap(),
            Value::Object(global)
        );
        let counts = runtime.heap_counts().expect("runtime state");
        assert_eq!(counts.context_nodes, 1);
        assert!(
            counts.var_ref_nodes > 0,
            "globalThis must use a global VarRef"
        );
    }

    assert_eq!(
        runtime.heap_counts().expect("runtime state").context_nodes,
        1
    );
    runtime.run_gc().unwrap();
    let counts = runtime.heap_counts().expect("runtime state");
    assert_eq!(counts.context_nodes, 0);
    assert_eq!(counts.object_nodes, 0);
    assert_eq!(counts.shape_nodes, 0);
    assert_eq!(counts.var_ref_nodes, 0);
    assert_eq!(counts.function_bytecode_nodes, 0);
    assert_eq!(counts.live, 0);
}

fn define_global_this(context: &mut Context, global: &ObjectRef, key: &PropertyKey) {
    assert!(
        context
            .define_own_property(
                global,
                key,
                &OrdinaryPropertyDescriptor {
                    value: DescriptorField::Present(Value::Object(
                        global.try_clone().expect("duplicate root")
                    )),
                    writable: DescriptorField::Present(true),
                    enumerable: DescriptorField::Present(false),
                    configurable: DescriptorField::Present(true),
                    ..OrdinaryPropertyDescriptor::new()
                },
            )
            .unwrap()
    );
}

fn function(runtime: &Runtime, context: &mut Context, source: &str) -> CallableRef {
    let Value::Object(object) = context.eval(source).unwrap() else {
        panic!("function source did not produce an object: {source}");
    };
    runtime.as_callable(&object).unwrap().unwrap()
}

fn reference_error_prototype(runtime: &Runtime, context: &mut Context) -> ObjectRef {
    let global = context.global_object().unwrap();
    let constructor_key = runtime.intern_property_key("ReferenceError").unwrap();
    let prototype_key = runtime.intern_property_key("prototype").unwrap();
    let Value::Object(constructor) = context.get_property(&global, &constructor_key).unwrap()
    else {
        panic!("ReferenceError was not an object");
    };
    let Value::Object(prototype) = context.get_property(&constructor, &prototype_key).unwrap()
    else {
        panic!("ReferenceError.prototype was not an object");
    };
    prototype
}
