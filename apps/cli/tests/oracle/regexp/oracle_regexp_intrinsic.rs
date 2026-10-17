use crate::runtime_completion_oracle::observe_read_context_eval_completion_with_prelude;
use crate::runtime_observation::host_runtime_context;
use quickjs_oxide::engine::api::core::{Context, Runtime};

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
