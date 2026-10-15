use crate::object_graph_observation::require_object as expect_object;
use crate::runtime_completion_oracle::observe_read_context_eval_completion_with_prelude;
use crate::runtime_observation::{host_runtime, host_runtime_context};
use crate::runtime_observation::{
    property_callable_with_read_context as property_callable, take_exception_object,
};
use crate::runtime_oracle::{eval_callable, eval_object};
use quickjs_oxide::engine::api::{CallableRef, Context, JsString, Runtime, RuntimeError, Value};

// Differential lock for observable RegExp semantics in pinned QuickJS
// 2026-06-04. The vectors deliberately stay below later String.prototype
// symbol hooks.

const PRELUDE: &str = r#"
function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+__bit(descriptor.configurable);
}
function __accessorBits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return (typeof descriptor.get)+":"+(descriptor.set===undefined)+":"+
        __bit(descriptor.enumerable)+__bit(descriptor.configurable);
}
function __isConstructor(value){
    try{Reflect.construct(function(){},[],value);return true}catch(_error){return false}
}
function __error(thunk){
    try{return "return:"+String(thunk())}
    catch(error){
        if(error!==null&&typeof error==="object")return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
function __show(value){
    if(value===undefined)return "undefined";
    if(value===null)return "null";
    if(typeof value==="number"){
        if(value!==value)return "NaN";
        if(value===0)return 1/value===-Infinity?"-0":"+0";
        if(value===Infinity)return "+Infinity";
        if(value===-Infinity)return "-Infinity";
    }
    return String(value);
}
function __callableMetadata(owner,key){
    var fn=owner[key];
    return String(key)+":"+fn.name+":"+fn.length+":"+__isConstructor(fn)+":"+
        __bits(owner,key)+":"+__bits(fn,"name")+":"+__bits(fn,"length");
}
"#;

const CALLABLE_PROXY_EXEC_SOURCE: &str = r#"(function(){
    var log=[];
    var exec=new Proxy(function(value){
        log.push("body:"+value);
        return null;
    },{
        apply:function(target,receiver,argumentsList){
            log.push("apply:"+(receiver.exec===exec));
            return Reflect.apply(target,receiver,argumentsList);
        }
    });
    var receiver={exec:exec};
    return [
        RegExp.prototype.test.call(receiver,"x"),
        log.join(",")
    ].join("|");
})()"#;

#[test]
fn regexp_literal_uses_the_bytecode_realm_and_is_fresh_on_every_execution() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");
    let defining_prototype = eval_object(
        &mut defining,
        "RegExp.prototype",
        "defining RegExp prototype",
    );
    let caller_prototype = eval_object(&mut caller, "RegExp.prototype", "caller RegExp prototype");
    let function = defining.compile("/realm/g").unwrap();

    // Mutating the constructor relationship after compilation must not affect
    // QuickJS's realm-canonical literal shape.
    drop(
        defining
            .eval("RegExp.prototype={replacement:true}")
            .unwrap(),
    );
    let first = expect_object(caller.execute(&function).unwrap(), "first RegExp literal");
    let second = expect_object(caller.execute(&function).unwrap(), "second RegExp literal");

    assert_ne!(first, second);
    assert_eq!(
        runtime.get_prototype_of(&first).unwrap(),
        Some(defining_prototype)
    );
    assert_ne!(
        runtime.get_prototype_of(&first).unwrap(),
        Some(caller_prototype)
    );
}

#[test]
fn regexp_test_calls_callable_proxy_exec_without_an_oracle() {
    let (runtime, mut context) = host_runtime_context();
    assert_eq!(
        observe_rust_eval(
            &runtime,
            &mut context,
            CALLABLE_PROXY_EXEC_SOURCE,
            "callable Proxy exec regression",
        ),
        "return|string|false|apply:true,body:x",
    );
}

#[test]
fn regexp_cross_realm_prototypes_results_fallback_and_errors_use_exact_realms() {
    let runtime = host_runtime();
    let mut defining = runtime.new_context().expect("create context");
    let mut caller = runtime.new_context().expect("create context");

    let defining_constructor = regexp_constructor(&runtime, &mut defining);
    let defining_regexp_prototype = eval_object(
        &mut defining,
        "RegExp.prototype",
        "defining RegExp prototype",
    );
    let caller_regexp_prototype =
        eval_object(&mut caller, "RegExp.prototype", "caller RegExp prototype");
    let defining_array_prototype =
        eval_object(&mut defining, "Array.prototype", "defining Array prototype");
    let caller_array_prototype =
        eval_object(&mut caller, "Array.prototype", "caller Array prototype");
    let defining_type_error = eval_object(
        &mut defining,
        "TypeError.prototype",
        "defining TypeError prototype",
    );
    let caller_type_error = eval_object(
        &mut caller,
        "TypeError.prototype",
        "caller TypeError prototype",
    );
    assert_ne!(defining_regexp_prototype, caller_regexp_prototype);
    assert_ne!(defining_array_prototype, caller_array_prototype);
    assert_ne!(defining_type_error, caller_type_error);

    let pattern = Value::String(JsString::try_from_utf8("a").unwrap());
    let flags = Value::String(JsString::try_from_utf8("g").unwrap());
    let foreign_regexp = expect_object(
        caller
            .construct(
                &defining_constructor,
                &[pattern.try_clone().expect("duplicate root"), flags],
            )
            .unwrap(),
        "foreign RegExp construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&foreign_regexp).unwrap(),
        Some(
            defining_regexp_prototype
                .try_clone()
                .expect("duplicate root")
        ),
        "RegExp construction did not use the constructor defining prototype",
    );

    let same = caller
        .call(
            &defining_constructor,
            Value::Undefined,
            &[Value::Object(
                foreign_regexp.try_clone().expect("duplicate root"),
            )],
        )
        .unwrap();
    assert_eq!(
        same,
        Value::Object(foreign_regexp),
        "foreign RegExp function call did not take the identity shortcut",
    );

    let caller_new_target = eval_callable(
        &runtime,
        &mut caller,
        "(function(){var F=(function(){}).bind(null);F.prototype=17;return F})()",
        "caller primitive-prototype newTarget",
    );
    let fallback_regexp = expect_object(
        caller
            .construct_with_new_target(&defining_constructor, &caller_new_target, &[pattern])
            .unwrap(),
        "cross-realm RegExp fallback construction",
    );
    assert_eq!(
        runtime.get_prototype_of(&fallback_regexp).unwrap(),
        Some(caller_regexp_prototype),
        "primitive newTarget.prototype did not fall back to the newTarget realm",
    );

    let exec = regexp_prototype_callable(&runtime, &mut defining, "exec");
    let caller_regexp = eval_object(&mut caller, "new RegExp('a')", "caller RegExp");
    let result = expect_object(
        caller
            .call(
                &exec,
                Value::Object(caller_regexp),
                &[Value::String(JsString::try_from_utf8("za").unwrap())],
            )
            .unwrap(),
        "foreign RegExp exec result",
    );
    assert_eq!(
        runtime.get_prototype_of(&result).unwrap(),
        Some(defining_array_prototype),
        "RegExp exec result did not use the builtin defining Array realm",
    );

    let ordinary = eval_object(&mut caller, "({})", "caller ordinary object");
    assert_eq!(
        caller.call(&exec, Value::Object(ordinary), &[]),
        Err(RuntimeError::Exception),
    );
    let native_error = take_exception_object(&mut caller, "foreign RegExp exec TypeError");
    assert_eq!(
        runtime.get_prototype_of(&native_error).unwrap(),
        Some(defining_type_error),
        "RegExp builtin native error used the calling realm",
    );

    let test = regexp_prototype_callable(&runtime, &mut defining, "test");
    let throwing = eval_object(
        &mut caller,
        r#"(function(){
            var receiver=Object();
            receiver.exec=function(){throw new TypeError("caller exec")};
            return receiver;
        })()"#,
        "caller throwing custom exec receiver",
    );
    assert_eq!(
        caller.call(&test, Value::Object(throwing), &[]),
        Err(RuntimeError::Exception),
    );
    let user_error = take_exception_object(&mut caller, "caller custom exec TypeError");
    assert_eq!(
        runtime.get_prototype_of(&user_error).unwrap(),
        Some(caller_type_error),
        "RegExp.test replaced a caller-realm user exception",
    );
}

fn observe_rust_eval(
    runtime: &Runtime,
    context: &mut Context,
    source: &str,
    description: &str,
) -> String {
    observe_read_context_eval_completion_with_prelude(
        runtime,
        context,
        PRELUDE,
        source,
        description,
    )
}

fn regexp_constructor(runtime: &Runtime, context: &mut Context) -> CallableRef {
    let global = context.global_object().unwrap();
    property_callable(runtime, context, &global, "RegExp")
}

fn regexp_prototype_callable(runtime: &Runtime, context: &mut Context, name: &str) -> CallableRef {
    let prototype = eval_object(context, "RegExp.prototype", "RegExp prototype");
    property_callable(runtime, context, &prototype, name)
}
