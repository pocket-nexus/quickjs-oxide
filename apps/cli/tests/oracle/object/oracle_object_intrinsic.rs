use crate::object_graph_observation::global_callable;
use crate::runtime_completion_oracle::observe_array_aware_eval as observe_rust_eval;
use crate::runtime_observation::property_callable_with_read_context as property_callable;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use quickjs_oxide::engine::api::{CallableRef, Context, Runtime, Value};

// This differential pins the first coherent `%Object%` vertical slice to
// QuickJS 2026-06-04. Source cases deliberately use only the selected static
// prefix (`create` through `getOwnPropertySymbols`), the complete
// `Object.prototype` table, and intrinsics which already precede this slice.
// Graph/descriptor reflection uses the host API on the Rust side so the test
// does not accidentally require the later Object.getOwnPropertyDescriptor
// static-table entry merely to observe this milestone.

const DEFINE_PROPERTIES_PROXY_ORDER: &str = r#"(function(){
    function run(create){
        var log=[],logger=new Proxy({},{
            get:function(target,key){log.push(String(key))}
        });
        var descriptors=new Proxy({a:{value:0},b:{value:1}},logger);
        if(create)Object.create(null,descriptors);
        else Object.defineProperties({},descriptors);
        return log.join(",");
    }
    return run(true)+"|"+run(false);
})()"#;

#[test]
fn object_define_properties_pins_quickjs_proxy_batch_order_without_an_oracle() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        observe_rust_eval(
            &runtime,
            &mut context,
            DEFINE_PROPERTIES_PROXY_ORDER,
            "Object.defineProperties Proxy batch order",
        ),
        "return|string|ownKeys,getOwnPropertyDescriptor,getOwnPropertyDescriptor,get,get|\
         ownKeys,getOwnPropertyDescriptor,getOwnPropertyDescriptor,get,get",
    );
}

#[test]
fn object_constructor_custom_new_target_and_cross_realm_fallback_are_pinned() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let constructor = global_callable(&runtime, &mut defining, "Object");
    let defining_object_prototype = defining.object_prototype().unwrap();
    let caller_object_prototype = caller.object_prototype().unwrap();
    let defining_boolean_prototype = defining.boolean_prototype().unwrap();
    let defining_array_prototype = defining.array_prototype().unwrap();

    let Value::Object(called) = caller
        .call(&constructor, Value::Undefined, &[])
        .expect("cross-realm Object call")
    else {
        panic!("Object() did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&called).unwrap().as_ref(),
        Some(&defining_object_prototype),
        "Object() did not allocate in its defining realm",
    );

    let Value::Object(constructed) = caller
        .construct(&constructor, &[])
        .expect("cross-realm new Object")
    else {
        panic!("new Object() did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&constructed).unwrap().as_ref(),
        Some(&defining_object_prototype),
        "new Object() did not allocate in its defining realm",
    );

    let Value::Object(boxed_boolean) = caller
        .call(&constructor, Value::Undefined, &[Value::Bool(true)])
        .expect("cross-realm Object Boolean boxing")
    else {
        panic!("Object(true) did not return a wrapper");
    };
    assert_eq!(
        runtime.get_prototype_of(&boxed_boolean).unwrap().as_ref(),
        Some(&defining_boolean_prototype),
        "Object primitive boxing did not use the constructor's defining realm",
    );

    let target = eval_callable(&runtime, &mut caller, "(function ForeignObjectTarget(){})");
    let prototype_key = runtime.intern_property_key("prototype").unwrap();
    let custom_prototype = caller.new_object().unwrap();
    assert!(
        caller
            .set_property(
                target.as_object(),
                &prototype_key,
                Value::Object(custom_prototype.try_clone().expect("duplicate root")),
            )
            .unwrap()
    );
    let Value::Object(custom) = caller
        .construct_with_new_target(&constructor, &target, &[Value::Int(7)])
        .expect("Object custom new.target prototype")
    else {
        panic!("Object custom new.target did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&custom).unwrap().as_ref(),
        Some(&custom_prototype),
        "Object custom new.target ignored its object prototype",
    );

    assert!(
        caller
            .set_property(target.as_object(), &prototype_key, Value::Int(1))
            .unwrap()
    );
    let Value::Object(fallback) = caller
        .construct_with_new_target(&constructor, &target, &[])
        .expect("Object custom new.target fallback")
    else {
        panic!("Object custom new.target fallback did not return an object");
    };
    assert_eq!(
        runtime.get_prototype_of(&fallback).unwrap().as_ref(),
        Some(&caller_object_prototype),
        "non-object new.target.prototype did not fall back to new.target's realm",
    );

    let names = property_callable(
        &runtime,
        &mut defining,
        constructor.as_object(),
        "getOwnPropertyNames",
    );
    let sample = caller.new_object().unwrap();
    let Value::Object(name_array) = caller
        .call(
            &names,
            Value::Undefined,
            &[Value::Object(sample.try_clone().expect("duplicate root"))],
        )
        .expect("cross-realm Object.getOwnPropertyNames")
    else {
        panic!("Object.getOwnPropertyNames did not return an Array");
    };
    assert_eq!(
        runtime.get_prototype_of(&name_array).unwrap().as_ref(),
        Some(&defining_array_prototype),
        "Object own-key result Array did not use the native's defining realm",
    );
}

fn eval_callable(runtime: &Runtime, context: &mut Context, source: &str) -> CallableRef {
    let Value::Object(function) = context.eval(source).expect("evaluate callable") else {
        panic!("callable source did not return an object");
    };
    runtime
        .as_callable(&function)
        .unwrap()
        .expect("evaluated object was not callable")
}
