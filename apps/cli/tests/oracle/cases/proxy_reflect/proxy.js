function __proxyValue(value) {
    if (value === undefined) return "undefined";
    if (value === null) return "null";
    if (typeof value === "symbol") return String(value);
    if (typeof value === "object" || typeof value === "function")
        return typeof value;
    if (typeof value === "number" && value === 0)
        return 1 / value < 0 ? "-0" : "+0";
    if (typeof value === "number" && value !== value) return "NaN";
    return String(value);
}
function __proxyCompletion(thunk) {
    try {
        return "return:" + __proxyValue(thunk());
    } catch (error) {
        if (error !== null &&
            (typeof error === "object" || typeof error === "function"))
            return "throw:" + error.name + ":" + error.message;
        return "throw:" + typeof error + ":" + String(error);
    }
}
function __proxyBits(descriptor) {
    if (descriptor === undefined) return "undefined";
    function bit(value) { return value ? 1 : 0; }
    var kind = Object.prototype.hasOwnProperty.call(descriptor, "value") ||
        Object.prototype.hasOwnProperty.call(descriptor, "writable") ? "d" : "a";
    var payload = kind === "d" ?
        __proxyValue(descriptor.value) + ":" + bit(descriptor.writable) :
        __proxyValue(descriptor.get) + ":" + __proxyValue(descriptor.set);
    return kind + ":" + payload + ":" + bit(descriptor.enumerable) +
        bit(descriptor.configurable);
}
function __proxyKey(key) {
    return typeof key === "symbol" ? String(key) : key;
}
function __proxyKeys(keys) {
    return keys.map(__proxyKey).join(",");
}
function __proxyIsConstructor(value) {
    try {
        Reflect.construct(function () {}, [], value);
        return true;
    } catch (_) {
        return false;
    }
}
// GRAPH_CASES
// case: Proxy constructor metadata global descriptor and method graph
(function(){
            var globalDescriptor=Object.getOwnPropertyDescriptor(globalThis,"Proxy");
            var revocableDescriptor=Object.getOwnPropertyDescriptor(Proxy,"revocable");
            var nameDescriptor=Object.getOwnPropertyDescriptor(Proxy,"name");
            var lengthDescriptor=Object.getOwnPropertyDescriptor(Proxy,"length");
            return [
                typeof Proxy,Proxy.name,Proxy.length,__proxyIsConstructor(Proxy),
                __proxyCompletion(function(){return Proxy({}, {})}),
                Object.prototype.hasOwnProperty.call(Proxy,"prototype"),
                Object.getPrototypeOf(Proxy)===Function.prototype,
                __proxyKeys(Reflect.ownKeys(Proxy)),
                globalDescriptor.writable,globalDescriptor.enumerable,
                globalDescriptor.configurable,
                revocableDescriptor.value===Proxy.revocable,
                revocableDescriptor.writable,revocableDescriptor.enumerable,
                revocableDescriptor.configurable,
                nameDescriptor.writable,nameDescriptor.enumerable,nameDescriptor.configurable,
                lengthDescriptor.writable,lengthDescriptor.enumerable,lengthDescriptor.configurable
            ].join("|");
        })()
// case: constructor requires object target and handler without coercion
(function(){
            var log="",coercible={
                valueOf:function(){log+="v";return {}},
                toString:function(){log+="s";return "[object Object]"}
            };
            return [
                __proxyCompletion(function(){return new Proxy({}, {})}),
                __proxyCompletion(function(){return new Proxy(1, {})}),
                __proxyCompletion(function(){return new Proxy({}, null)}),
                __proxyCompletion(function(){return new Proxy(coercible, {})}),
                __proxyCompletion(function(){return new Proxy({}, coercible)}),
                log
            ].join("|");
        })()
// case: revocable result and one-shot revoke function metadata
(function(){
            var pair=Proxy.revocable({answer:42},{});
            var proxyDescriptor=Object.getOwnPropertyDescriptor(pair,"proxy");
            var revokeDescriptor=Object.getOwnPropertyDescriptor(pair,"revoke");
            return [
                Object.getPrototypeOf(pair)===Object.prototype,
                __proxyKeys(Reflect.ownKeys(pair)),
                pair.proxy===proxyDescriptor.value,pair.revoke===revokeDescriptor.value,
                proxyDescriptor.writable,proxyDescriptor.enumerable,proxyDescriptor.configurable,
                revokeDescriptor.writable,revokeDescriptor.enumerable,revokeDescriptor.configurable,
                typeof pair.revoke,pair.revoke.name,pair.revoke.length,
                __proxyIsConstructor(pair.revoke),
                Object.getPrototypeOf(pair.revoke)===Function.prototype,
                __proxyKeys(Object.getOwnPropertyNames(pair.revoke))
            ].join("|");
        })()
// LIFECYCLE_CASES
// case: revocation is idempotent ignores this and arguments and severs the closure capture
(function(){
            var pair=Proxy.revocable({answer:42},{}),proxy=pair.proxy,revoke=pair.revoke;
            var before=proxy.answer;
            var first=revoke.call({ignored:true},1,2);
            var second=revoke.call(null,3);
            return [
                before,first===undefined,second===undefined,
                __proxyCompletion(function(){return proxy.answer}),
                __proxyCompletion(function(){return revoke()}),
                typeof proxy,typeof revoke
            ].join("|");
        })()
// case: every internal method rejects a revoked callable constructor proxy
(function(){
            function Target(value){this.value=value;return value+1}
            var pair=Proxy.revocable(Target,{}),proxy=pair.proxy;
            pair.revoke();
            var operations=[
                function(){return Reflect.getPrototypeOf(proxy)},
                function(){return Reflect.setPrototypeOf(proxy,null)},
                function(){return Reflect.isExtensible(proxy)},
                function(){return Reflect.preventExtensions(proxy)},
                function(){return Reflect.has(proxy,"x")},
                function(){return Reflect.get(proxy,"x")},
                function(){return Reflect.set(proxy,"x",1)},
                function(){return Reflect.getOwnPropertyDescriptor(proxy,"x")},
                function(){return Reflect.defineProperty(proxy,"x",{value:1})},
                function(){return Reflect.deleteProperty(proxy,"x")},
                function(){return Reflect.ownKeys(proxy)},
                function(){return Reflect.apply(proxy,null,[1])},
                function(){return Reflect.construct(proxy,[1])}
            ];
            return operations.map(__proxyCompletion).join("|");
        })()
// case: revoked operations do not read any handler trap getter
(function(){
            var log="",handler={},names=[
                "getPrototypeOf","setPrototypeOf","isExtensible","preventExtensions",
                "has","get","set","getOwnPropertyDescriptor","defineProperty",
                "deleteProperty","ownKeys","apply","construct"
            ];
            names.forEach(function(name){
                Object.defineProperty(handler,name,{get:function(){log+=name+";";return undefined}});
            });
            var pair=Proxy.revocable(function(){},handler),proxy=pair.proxy;
            pair.revoke();
            var results=[
                __proxyCompletion(function(){return proxy.x}),
                __proxyCompletion(function(){return Reflect.ownKeys(proxy)}),
                __proxyCompletion(function(){return proxy()}),
                __proxyCompletion(function(){return new proxy()})
            ];
            return results.join("|")+"|log:"+log;
        })()
// case: revocation preserves cached callable and constructable identity tests
(function(){
            function Target(){}
            var pair=Proxy.revocable(Target,{}),proxy=pair.proxy;
            var before=[typeof proxy,__proxyIsConstructor(proxy)];
            pair.revoke();
            return before.join(":")+"|"+
                [typeof proxy,__proxyIsConstructor(proxy),
                 __proxyCompletion(function(){return proxy()}),
                 __proxyCompletion(function(){return new proxy()})].join(":");
        })()
// FALLBACK_CASES
// case: null traps forward all eleven object internal methods
(function(){
            var target={x:1},proto={inherited:7},handler={
                getPrototypeOf:null,setPrototypeOf:null,isExtensible:null,
                preventExtensions:null,has:null,get:null,set:null,
                getOwnPropertyDescriptor:null,defineProperty:null,
                deleteProperty:null,ownKeys:null
            };
            var proxy=new Proxy(target,handler),nextProto={next:8};
            var initialProto=Reflect.getPrototypeOf(proxy)===Object.prototype;
            var changed=Reflect.setPrototypeOf(proxy,nextProto);
            var inherited=proxy.next;
            var extensible=Reflect.isExtensible(proxy);
            var has=Reflect.has(proxy,"x"),got=Reflect.get(proxy,"x");
            var set=Reflect.set(proxy,"x",2);
            var defined=Reflect.defineProperty(proxy,"y",{
                value:3,writable:true,enumerable:true,configurable:true
            });
            var descriptor=Reflect.getOwnPropertyDescriptor(proxy,"y");
            var keys=__proxyKeys(Reflect.ownKeys(proxy));
            var deleted=Reflect.deleteProperty(proxy,"y");
            var prevented=Reflect.preventExtensions(proxy);
            return [
                initialProto,changed,inherited,extensible,has,got,set,target.x,
                defined,__proxyBits(descriptor),keys,deleted,!Object.hasOwn(target,"y"),
                prevented,Reflect.isExtensible(target)
            ].join("|");
        })()
// case: undefined and null apply construct traps preserve this arguments and newTarget
(function(){
            function Target(a,b){
                if(new.target){
                    this.sum=a+b;this.seen=new.target;return;
                }
                return this.base+a+b;
            }
            var proxy=new Proxy(Target,{apply:null,construct:undefined});
            function NewTarget(){}
            var proto={kind:"new-target"};NewTarget.prototype=proto;
            var called=Reflect.apply(proxy,{base:2},[19,21]);
            var made=Reflect.construct(proxy,[20,22],NewTarget);
            return [
                called,made.sum,made.seen===NewTarget,
                Object.getPrototypeOf(made)===proto,made.kind,
                typeof proxy,__proxyIsConstructor(proxy)
            ].join("|");
        })()
// case: missing mutation traps preserve target rejection errors
(function(){
            var frozen={};
            Object.defineProperty(frozen,"x",{
                value:1,writable:false,configurable:false
            });
            var setProxy=new Proxy(frozen,{});
            function strictSet(){"use strict";setProxy.x=2}
            var closed=Object.preventExtensions({});
            var defineProxy=new Proxy(closed,{});
            return [
                __proxyCompletion(strictSet),
                __proxyCompletion(function(){
                    return Object.defineProperty(defineProxy,"x",{value:1});
                })
            ].join("|");
        })()
// case: empty-handler forwarding retains pinned finite stack overflow behavior
(function(){
            function wrap(count){
                var value={x:42};
                for(var index=0;index<count;index++)
                    value=new Proxy(value,{});
                return value;
            }
            var admitted=wrap(1500),overflow=wrap(3000);
            return [
                admitted.x,
                __proxyCompletion(function(){return overflow.x}),
                ({x:43}).x
            ].join("|");
        })()
// case: empty-handler forwarding preserves operation-specific pinned QuickJS stack behavior
(function(){
            function wrap(value,count){
                var handler={};
                while(count--)
                    value=new Proxy(value,handler);
                return value;
            }
            var callable=wrap(function(){return 42},2000);
            var object=wrap({},2518);
            return [
                __proxyCompletion(function(){return callable()}),
                __proxyCompletion(function(){return Reflect.getPrototypeOf(object)})
            ].join("|");
        })()
// case: Iterator toStringTag setter dispatches Proxy descriptor and define traps
(function(){
            var log=[],target=Object.create(Iterator.prototype);
            var proxy=new Proxy(target,{
                getOwnPropertyDescriptor:function(seen,key){
                    log.push("gopd:"+String(key));
                    return Reflect.getOwnPropertyDescriptor(seen,key);
                },
                defineProperty:function(seen,key,descriptor){
                    log.push("define:"+String(key)+":"+descriptor.value);
                    return Reflect.defineProperty(seen,key,descriptor);
                }
            });
            proxy[Symbol.toStringTag]="X";
            return [log.join("|"),Object.prototype.toString.call(proxy)].join("|");
        })()
// case: Function bind observes Proxy HasOwnProperty before length and name Get
(function(){
            var log=[];
            function target(a,b){}
            var proxy=new Proxy(target,{
                getOwnPropertyDescriptor:function(seen,key){
                    log.push("gopd:"+key);
                    return Reflect.getOwnPropertyDescriptor(seen,key);
                },
                get:function(seen,key,receiver){
                    log.push("get:"+key);
                    return Reflect.get(seen,key,receiver);
                }
            });
            var bound=Function.prototype.bind.call(proxy,null,1);
            return [bound.length,bound.name,log.join("|")].join("|");
        })()
// TRAP_CASES
// case: getPrototypeOf and setPrototypeOf receive handler target and requested prototype
(function(){
            var log=[],target={},reported={reported:1},requested={requested:1},handler={
                getPrototypeOf:function(seen){
                    log.push("get:"+(this===handler)+":"+(seen===target));return reported;
                },
                setPrototypeOf:function(seen,proto){
                    log.push("set:"+(this===handler)+":"+(seen===target)+":"+(proto===requested));
                    return "truthy";
                }
            };
            var proxy=new Proxy(target,handler);
            return [
                Reflect.getPrototypeOf(proxy)===reported,
                Reflect.setPrototypeOf(proxy,requested),
                Object.getPrototypeOf(target)===Object.prototype,
                log.join(",")
            ].join("|");
        })()
// case: isExtensible and preventExtensions receive the raw target and coerce trap results
(function(){
            var log=[],target={},handler={
                isExtensible:function(seen){
                    log.push("is:"+(this===handler)+":"+(seen===target));
                    return Reflect.isExtensible(seen)?1:0;
                },
                preventExtensions:function(seen){
                    log.push("prevent:"+(this===handler)+":"+(seen===target));
                    Reflect.preventExtensions(seen);return {};
                }
            };
            var proxy=new Proxy(target,handler);
            return [
                Reflect.isExtensible(proxy),Reflect.preventExtensions(proxy),
                Reflect.isExtensible(proxy),log.join(",")
            ].join("|");
        })()
// case: descriptor traps receive property keys and only the requested descriptor fields
(function(){
            var log=[],target={a:1},handler={
                getOwnPropertyDescriptor:function(seen,key){
                    log.push("get:"+(this===handler)+":"+(seen===target)+":"+key);
                    return Reflect.getOwnPropertyDescriptor(seen,key);
                },
                defineProperty:function(seen,key,descriptor){
                    log.push("define:"+(this===handler)+":"+(seen===target)+":"+key+":"+
                        Reflect.ownKeys(descriptor).join(","));
                    return Reflect.defineProperty(seen,key,descriptor);
                }
            };
            var proxy=new Proxy(target,handler);
            var defined=Reflect.defineProperty(proxy,"b",{value:2,enumerable:true});
            var descriptor=Reflect.getOwnPropertyDescriptor(proxy,"b");
            return [defined,__proxyBits(descriptor),log.join("|")].join("|");
        })()
// case: has get set and delete traps preserve key value and explicit receiver
(function(){
            var log=[],target={x:20},receiver={marker:"receiver"},handler={
                has:function(seen,key){
                    log.push("has:"+(this===handler)+":"+(seen===target)+":"+key);
                    return Reflect.has(seen,key);
                },
                get:function(seen,key,seenReceiver){
                    log.push("get:"+(this===handler)+":"+(seen===target)+":"+key+":"+
                        (seenReceiver===receiver));
                    return Reflect.get(seen,key,seenReceiver)+2;
                },
                set:function(seen,key,value,seenReceiver){
                    log.push("set:"+(this===handler)+":"+(seen===target)+":"+key+":"+value+":"+
                        (seenReceiver===receiver));
                    seenReceiver[key]=value;return true;
                },
                deleteProperty:function(seen,key){
                    log.push("delete:"+(this===handler)+":"+(seen===target)+":"+key);
                    return Reflect.deleteProperty(seen,key);
                }
            };
            var proxy=new Proxy(target,handler);
            return [
                Reflect.has(proxy,"x"),Reflect.get(proxy,"x",receiver),
                Reflect.set(proxy,"y",42,receiver),receiver.y,
                Reflect.deleteProperty(proxy,"x"),Object.hasOwn(target,"x"),
                log.join("|")
            ].join("|");
        })()
// case: ownKeys receives target and preserves arbitrary string symbol order
(function(){
            var symbol=Symbol("s"),target={a:1},log="",handler={
                ownKeys:function(seen){
                    log+=(this===handler)+":"+(seen===target);
                    return [symbol,"z","a"];
                }
            };
            var proxy=new Proxy(target,handler);
            return __proxyKeys(Reflect.ownKeys(proxy))+"|"+log;
        })()
// case: apply receives target thisArg and a fresh dense argument array
(function(){
            function Target(){return "target"}
            var receiver={marker:7},captured,handler={
                apply:function(seen,thisArg,args){
                    captured=args;
                    return [
                        this===handler,seen===Target,thisArg===receiver,
                        Array.isArray(args),args.length,args[0],args[1],
                        Object.getPrototypeOf(args)===Array.prototype
                    ].join(":");
                }
            };
            var proxy=new Proxy(Target,handler),result=Reflect.apply(proxy,receiver,[20,22]);
            return result+"|"+(captured!==arguments);
        })()
// case: construct receives target dense arguments and exact custom newTarget
(function(){
            function Target(){}
            function NewTarget(){}
            var captured,firstCaptured,handler={
                construct:function(seen,args,newTarget){
                    captured=args;
                    if(args.length===2)firstCaptured=args;
                    return {
                        summary:[
                            this===handler,seen===Target,newTarget===NewTarget,
                            Array.isArray(args),args.length,args[0],args[1],
                            Object.getPrototypeOf(args)===Array.prototype
                        ].join(":")
                    };
                }
            };
            var proxy=new Proxy(Target,handler);
            var value=Reflect.construct(proxy,[20,22],NewTarget);
            var direct=new proxy(1);
            return value.summary+"|"+(firstCaptured.length===2)+":"+
                (firstCaptured!==captured)+"|"+
                __proxyValue(direct.summary)+":"+(direct.summary.indexOf("false")<0);
        })()
// case: trap getter is read before invocation and a noncallable trap is rejected
(function(){
            var log=[],target={x:42},handler={};
            Object.defineProperty(handler,"get",{get:function(){
                log.push("getter");
                return function(seen,key,receiver){
                    log.push("call:"+(this===handler)+":"+(seen===target)+":"+key+
                        ":"+(receiver===proxy));
                    return 42;
                };
            },configurable:true});
            var proxy=new Proxy(target,handler),first=proxy.x;
            Object.defineProperty(handler,"get",{value:1,configurable:true});
            var second=__proxyCompletion(function(){return proxy.x});
            return [first,second,log.join("|")].join("|");
        })()
// case: false mutation traps feed Reflect booleans and throwing language or Object APIs
(function(){
            var target={x:1},handler={
                set:function(){return false},
                defineProperty:function(){return false},
                deleteProperty:function(){return false}
            };
            var proxy=new Proxy(target,handler);
            function strictSet(){"use strict";proxy.x=2}
            return [
                __proxyCompletion(function(){return Reflect.set(proxy,"x",2)}),target.x,
                __proxyCompletion(strictSet),target.x,
                __proxyCompletion(function(){
                    return Reflect.defineProperty(proxy,"y",{value:2});
                }),
                __proxyCompletion(function(){
                    return Object.defineProperty(proxy,"y",{value:2});
                }),
                __proxyCompletion(function(){return Reflect.deleteProperty(proxy,"x")}),
                __proxyCompletion(function(){return delete proxy.x}),target.x
            ].join("|");
        })()
// case: ownKeys consumes an array-like trap result length then indices in order
(function(){
            var log=[],symbol=Symbol("s"),result={};
            Object.defineProperty(result,"length",{get:function(){log.push("length");return 2}});
            Object.defineProperty(result,"0",{get:function(){log.push("0");return "a"}});
            Object.defineProperty(result,"1",{get:function(){log.push("1");return symbol}});
            var proxy=new Proxy({a:1},{ownKeys:function(){log.push("trap");return result}});
            return __proxyKeys(Reflect.ownKeys(proxy))+"|"+log.join(",");
        })()
// INVARIANT_CASES
// case: getPrototypeOf rejects primitives and mismatches on nonextensible targets
(function(){
            var proto={},other={},target=Object.create(proto);
            var primitive=new Proxy(target,{getPrototypeOf:function(){return 1}});
            Object.preventExtensions(target);
            var mismatch=new Proxy(target,{getPrototypeOf:function(){return other}});
            var matching=new Proxy(target,{getPrototypeOf:function(){return proto}});
            return [
                __proxyCompletion(function(){return Reflect.getPrototypeOf(primitive)}),
                __proxyCompletion(function(){return Reflect.getPrototypeOf(mismatch)}),
                __proxyCompletion(function(){return Reflect.getPrototypeOf(matching)===proto})
            ].join("|");
        })()
// case: setPrototypeOf false is observable and true cannot mismatch a nonextensible target
(function(){
            var proto={},other={},target=Object.preventExtensions(Object.create(proto));
            var rejecting=new Proxy(target,{setPrototypeOf:function(){return 0}});
            var mismatch=new Proxy(target,{setPrototypeOf:function(){return true}});
            var matching=new Proxy(target,{setPrototypeOf:function(seen,next){return next===proto}});
            return [
                __proxyCompletion(function(){return Reflect.setPrototypeOf(rejecting,other)}),
                __proxyCompletion(function(){return Object.setPrototypeOf(rejecting,other)}),
                __proxyCompletion(function(){return Reflect.setPrototypeOf(mismatch,other)}),
                __proxyCompletion(function(){return Reflect.setPrototypeOf(matching,proto)})
            ].join("|");
        })()
// case: isExtensible trap result must equal the target state in both directions
(function(){
            var open={},closed=Object.preventExtensions({});
            var cases=[
                new Proxy(open,{isExtensible:function(){return false}}),
                new Proxy(closed,{isExtensible:function(){return true}}),
                new Proxy(open,{isExtensible:function(){return true}}),
                new Proxy(closed,{isExtensible:function(){return false}})
            ];
            return cases.map(function(proxy){
                return __proxyCompletion(function(){return Reflect.isExtensible(proxy)});
            }).join("|");
        })()
// case: preventExtensions may report false but true requires a closed target
(function(){
            var open1={},open2={},open3={};
            var falseTrap=new Proxy(open1,{preventExtensions:function(){return false}});
            var lying=new Proxy(open2,{preventExtensions:function(){return true}});
            var closing=new Proxy(open3,{preventExtensions:function(target){
                Object.preventExtensions(target);return true;
            }});
            return [
                __proxyCompletion(function(){return Reflect.preventExtensions(falseTrap)}),
                Object.isExtensible(open1),
                __proxyCompletion(function(){return Reflect.preventExtensions(lying)}),
                Object.isExtensible(open2),
                __proxyCompletion(function(){return Reflect.preventExtensions(closing)}),
                Object.isExtensible(open3)
            ].join("|");
        })()
// case: has cannot hide nonconfigurable or nonextensible own properties
(function(){
            var fixed={},closed={x:1},open={x:1};
            Object.defineProperty(fixed,"x",{value:1,configurable:false});
            Object.preventExtensions(closed);
            var fixedProxy=new Proxy(fixed,{has:function(){return false}});
            var closedProxy=new Proxy(closed,{has:function(){return false}});
            var openProxy=new Proxy(open,{has:function(){return false}});
            return [
                __proxyCompletion(function(){return Reflect.has(fixedProxy,"x")}),
                __proxyCompletion(function(){return Reflect.has(closedProxy,"x")}),
                __proxyCompletion(function(){return Reflect.has(openProxy,"x")}),
                Object.hasOwn(open,"x")
            ].join("|");
        })()
// case: get preserves SameValue for frozen data including NaN and signed zero
(function(){
            var one={},nan={},zero={};
            Object.defineProperty(one,"x",{value:1,writable:false,configurable:false});
            Object.defineProperty(nan,"x",{value:NaN,writable:false,configurable:false});
            Object.defineProperty(zero,"x",{value:-0,writable:false,configurable:false});
            return [
                __proxyCompletion(function(){return new Proxy(one,{get:function(){return 2}}).x}),
                __proxyCompletion(function(){return new Proxy(one,{get:function(){return 1}}).x}),
                __proxyCompletion(function(){return new Proxy(nan,{get:function(){return NaN}}).x}),
                __proxyCompletion(function(){return new Proxy(zero,{get:function(){return +0}}).x}),
                __proxyCompletion(function(){return new Proxy(zero,{get:function(){return -0}}).x})
            ].join("|");
        })()
// case: get cannot invent a value for a frozen accessor without a getter
(function(){
            var target={};
            Object.defineProperty(target,"x",{get:undefined,set:function(){},configurable:false});
            var lying=new Proxy(target,{get:function(){return 1}});
            var matching=new Proxy(target,{get:function(){return undefined}});
            return [
                __proxyCompletion(function(){return lying.x}),
                __proxyCompletion(function(){return matching.x})
            ].join("|");
        })()
// case: set cannot report success for an incompatible frozen data write
(function(){
            var target={};
            Object.defineProperty(target,"x",{value:1,writable:false,configurable:false});
            var proxy=new Proxy(target,{set:function(){return true}});
            return [
                __proxyCompletion(function(){return Reflect.set(proxy,"x",2)}),
                __proxyCompletion(function(){return Reflect.set(proxy,"x",1)}),
                target.x
            ].join("|");
        })()
// case: set cannot report success for a frozen accessor without a setter
(function(){
            var target={};
            Object.defineProperty(target,"x",{get:function(){return 1},set:undefined,configurable:false});
            var proxy=new Proxy(target,{set:function(){return true}});
            return __proxyCompletion(function(){return Reflect.set(proxy,"x",2)});
        })()
// case: getOwnPropertyDescriptor requires object or undefined and cannot hide fixed properties
(function(){
            var fixed={},closed={x:1},open={x:1};
            Object.defineProperty(fixed,"x",{value:1,configurable:false});
            Object.preventExtensions(closed);
            var primitive=new Proxy(open,{getOwnPropertyDescriptor:function(){return 1}});
            var hideFixed=new Proxy(fixed,{getOwnPropertyDescriptor:function(){return undefined}});
            var hideClosed=new Proxy(closed,{getOwnPropertyDescriptor:function(){return undefined}});
            var hideOpen=new Proxy(open,{getOwnPropertyDescriptor:function(){return undefined}});
            return [
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(primitive,"x")}),
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(hideFixed,"x")}),
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(hideClosed,"x")}),
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(hideOpen,"x")})
            ].join("|");
        })()
// case: getOwnPropertyDescriptor cannot add to a closed target or invent fixed state
(function(){
            var closed=Object.preventExtensions({}),open={},configurable={x:1};
            var descriptor={value:1,writable:true,enumerable:true,configurable:true};
            var addClosed=new Proxy(closed,{getOwnPropertyDescriptor:function(){return descriptor}});
            var addFixed=new Proxy(open,{getOwnPropertyDescriptor:function(){
                return {value:1,writable:true,enumerable:true,configurable:false};
            }});
            var tighten=new Proxy(configurable,{getOwnPropertyDescriptor:function(){
                return {value:1,writable:true,enumerable:true,configurable:false};
            }});
            return [
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(addClosed,"x")}),
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(addFixed,"x")}),
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(tighten,"x")})
            ].join("|");
        })()
// case: getOwnPropertyDescriptor enforces frozen kind writable and flag compatibility
(function(){
            var data={},accessor={},getter=function(){return 1};
            Object.defineProperty(data,"x",{value:1,writable:false,enumerable:false,configurable:false});
            Object.defineProperty(accessor,"x",{get:getter,enumerable:false,configurable:false});
            var writable=new Proxy(data,{getOwnPropertyDescriptor:function(){
                return {value:1,writable:true,enumerable:false,configurable:false};
            }});
            var enumerable=new Proxy(data,{getOwnPropertyDescriptor:function(){
                return {value:1,writable:false,enumerable:true,configurable:false};
            }});
            var kind=new Proxy(accessor,{getOwnPropertyDescriptor:function(){
                return {value:1,writable:false,enumerable:false,configurable:false};
            }});
            return [
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(writable,"x")}),
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(enumerable,"x")}),
                __proxyCompletion(function(){return Object.getOwnPropertyDescriptor(kind,"x")})
            ].join("|");
        })()
// case: getOwnPropertyDescriptor completes omitted flags before publishing the result
(function(){
            var target={};
            Object.defineProperty(target,"x",{
                value:42,writable:false,enumerable:false,configurable:false
            });
            var proxy=new Proxy(target,{getOwnPropertyDescriptor:function(){
                return {value:42};
            }});
            var descriptor=Object.getOwnPropertyDescriptor(proxy,"x");
            return [
                __proxyBits(descriptor),Reflect.ownKeys(descriptor).join(","),
                descriptor.value,descriptor.writable,
                descriptor.enumerable,descriptor.configurable
            ].join("|");
        })()
// case: defineProperty cannot add to closed targets or fabricate nonconfigurable properties
(function(){
            var closed=Object.preventExtensions({}),open={},configurable={x:1};
            var addClosed=new Proxy(closed,{defineProperty:function(){return true}});
            var addFixed=new Proxy(open,{defineProperty:function(){return true}});
            var tighten=new Proxy(configurable,{defineProperty:function(){return true}});
            return [
                __proxyCompletion(function(){
                    return Reflect.defineProperty(addClosed,"x",{value:1});
                }),
                __proxyCompletion(function(){
                    return Reflect.defineProperty(addFixed,"x",{value:1,configurable:false});
                }),
                __proxyCompletion(function(){
                    return Reflect.defineProperty(tighten,"x",{configurable:false});
                })
            ].join("|");
        })()
// case: defineProperty enforces frozen data accessor identity and writable tightening
(function(){
            var frozen={},accessor={},writable={};
            var getter=function(){return 1},other=function(){return 2};
            Object.defineProperty(frozen,"x",{value:1,writable:false,configurable:false});
            Object.defineProperty(accessor,"x",{get:getter,configurable:false});
            Object.defineProperty(writable,"x",{value:1,writable:true,configurable:false});
            var dataProxy=new Proxy(frozen,{defineProperty:function(){return true}});
            var accessorProxy=new Proxy(accessor,{defineProperty:function(){return true}});
            var writableProxy=new Proxy(writable,{defineProperty:function(){return true}});
            return [
                __proxyCompletion(function(){
                    return Reflect.defineProperty(dataProxy,"x",{value:2});
                }),
                __proxyCompletion(function(){
                    return Reflect.defineProperty(accessorProxy,"x",{get:other});
                }),
                __proxyCompletion(function(){
                    return Reflect.defineProperty(writableProxy,"x",{writable:false});
                }),
                __proxyCompletion(function(){
                    return Reflect.defineProperty(dataProxy,"x",{value:1});
                })
            ].join("|");
        })()
// case: deleteProperty cannot hide nonconfigurable or nonextensible own properties
(function(){
            var fixed={},closed={x:1},open={x:1};
            Object.defineProperty(fixed,"x",{value:1,configurable:false});
            Object.preventExtensions(closed);
            var fixedProxy=new Proxy(fixed,{deleteProperty:function(){return true}});
            var closedProxy=new Proxy(closed,{deleteProperty:function(){return true}});
            var openProxy=new Proxy(open,{deleteProperty:function(){return true}});
            var falseProxy=new Proxy(open,{deleteProperty:function(){return false}});
            return [
                __proxyCompletion(function(){return Reflect.deleteProperty(fixedProxy,"x")}),
                __proxyCompletion(function(){return Reflect.deleteProperty(closedProxy,"x")}),
                __proxyCompletion(function(){return Reflect.deleteProperty(openProxy,"x")}),
                Object.hasOwn(open,"x"),
                __proxyCompletion(function(){return Reflect.deleteProperty(falseProxy,"x")})
            ].join("|");
        })()
// case: ownKeys accepts only strings and symbols and rejects duplicates
(function(){
            var target={},symbol=Symbol("s"),cases=[
                new Proxy(target,{ownKeys:function(){return ["a",1]}}),
                new Proxy(target,{ownKeys:function(){return ["a","a"]}}),
                new Proxy(target,{ownKeys:function(){return [symbol,symbol]}}),
                new Proxy(target,{ownKeys:function(){return ["a",symbol]}})
            ];
            return cases.map(function(proxy){
                return __proxyCompletion(function(){return __proxyKeys(Reflect.ownKeys(proxy))});
            }).join("|");
        })()
// case: ownKeys must include fixed keys and exactly match a nonextensible target
(function(){
            var fixed={},closed={a:1},open={a:1};
            Object.defineProperty(fixed,"x",{value:1,configurable:false});
            Object.preventExtensions(closed);
            var missFixed=new Proxy(fixed,{ownKeys:function(){return []}});
            var missClosed=new Proxy(closed,{ownKeys:function(){return []}});
            var extraClosed=new Proxy(closed,{ownKeys:function(){return ["a","extra"]}});
            var extraOpen=new Proxy(open,{ownKeys:function(){return ["extra","a"]}});
            return [
                __proxyCompletion(function(){return Reflect.ownKeys(missFixed)}),
                __proxyCompletion(function(){return Reflect.ownKeys(missClosed)}),
                __proxyCompletion(function(){return Reflect.ownKeys(extraClosed)}),
                __proxyCompletion(function(){return __proxyKeys(Reflect.ownKeys(extraOpen))})
            ].join("|");
        })()
// case: ownKeys preserves a large reverse exact result
(function(){
            var count=4096,target={},keys=[];
            for(var index=0;index<count;index++){
                var key="reverse-"+index;
                keys.push(key);
                Object.defineProperty(target,key,{value:index,configurable:index%2===0});
            }
            Object.preventExtensions(target);
            var proxy=new Proxy(target,{ownKeys:function(){return keys.slice().reverse()}});
            var result=Reflect.ownKeys(proxy);
            return [result.length,result[0],result[result.length-1]].join("|");
        })()
// case: construct trap must return an object and cannot make a nonconstructor constructable
(function(){
            function Constructor(){}
            var primitive=new Proxy(Constructor,{construct:function(){return 1}});
            var nonConstructor=({method(){}}).method;
            var nonconstructor=new Proxy(nonConstructor,{construct:function(){return {made:true}}});
            return [
                __proxyCompletion(function(){return Reflect.construct(primitive,[])}),
                __proxyIsConstructor(nonconstructor),
                __proxyCompletion(function(){return Reflect.construct(nonconstructor,[])})
            ].join("|");
        })()
// QUIRK_CASES
// case: pinned ownKeys rechecks revocation after the trap returns
(function(){
            var pair,target={x:1};
            Object.defineProperty(target,"fixed",{value:2,configurable:false});
            pair=Proxy.revocable(target,{ownKeys:function(){
                pair.revoke();return ["x","fixed"];
            }});
            return __proxyCompletion(function(){return __proxyKeys(Reflect.ownKeys(pair.proxy))});
        })()
// case: pinned getOwnPropertyDescriptor frozen checks omit value getter and setter identity
(function(){
            var data={},accessor={},first=function(){return 1},second=function(){return 2};
            var firstSetter=function(){},secondSetter=function(){};
            Object.defineProperty(data,"x",{
                value:1,writable:false,enumerable:false,configurable:false
            });
            Object.defineProperty(accessor,"x",{
                get:first,set:firstSetter,enumerable:false,configurable:false
            });
            var dataProxy=new Proxy(data,{getOwnPropertyDescriptor:function(){
                return {value:2,writable:false,enumerable:false,configurable:false};
            }});
            var accessorProxy=new Proxy(accessor,{getOwnPropertyDescriptor:function(){
                return {get:second,set:secondSetter,enumerable:false,configurable:false};
            }});
            var dataDescriptor=Object.getOwnPropertyDescriptor(dataProxy,"x");
            var accessorDescriptor=Object.getOwnPropertyDescriptor(accessorProxy,"x");
            return [
                dataDescriptor.value,dataDescriptor.value!==data.x,
                accessorDescriptor.get===second,accessorDescriptor.get!==first,
                accessorDescriptor.set===secondSetter,accessorDescriptor.set!==firstSetter
            ].join("|");
        })()
// case: pinned noncallable Proxy reads apply before reporting not a function
(function(){
            var log="",handler={};
            Object.defineProperty(handler,"apply",{get:function(){
                log+="get;";
                return function(){log+="call;";return 42};
            }});
            var proxy=new Proxy({},handler);
            var result=__proxyCompletion(function(){return proxy()});
            return [typeof proxy,result,log].join("|");
        })()
// case: pinned ownKeys reads every entry before reporting duplicates
(function(){
            var log=[],result={length:3,0:"a",1:"a"};
            Object.defineProperty(result,"2",{get:function(){
                log.push("later");throw 42;
            }});
            var proxy=new Proxy({},{ownKeys:function(){return result}});
            return [
                __proxyCompletion(function(){return Reflect.ownKeys(proxy)}),
                log.join(",")
            ].join("|");
        })()
// case: pinned trap dispatch enters a noncallable Proxy before rejecting it
(function(){
            var log="",trapHandler={};
            Object.defineProperty(trapHandler,"apply",{get:function(){
                log+="get;";return function(){log+="call;";return 1};
            }});
            var trap=new Proxy({},trapHandler);
            var proxy=new Proxy({x:42},{get:trap});
            return [
                __proxyCompletion(function(){return proxy.x}),
                log
            ].join("|");
        })()
// case: GetFunctionRealm reports revocation after an observable prototype Get
(function(){
            var pair;
            pair=Proxy.revocable(function(){},{
                get:function(target,key,receiver){
                    if(key==="prototype"){pair.revoke();return null}
                    return Reflect.get(target,key,receiver);
                }
            });
            return __proxyCompletion(function(){
                return Reflect.construct(function(){},[],pair.proxy);
            });
        })()
// case: pinned nested Proxy undefined descriptor trap bypasses target IsExtensible
(function(){
            var log=[],base={x:1};
            var inner=new Proxy(base,{
                getOwnPropertyDescriptor:function(target,key){
                    log.push("inner-descriptor:"+key);
                    return Reflect.getOwnPropertyDescriptor(target,key);
                },
                isExtensible:function(target){
                    log.push("inner-extensible");
                    return Reflect.isExtensible(target);
                }
            });
            var outer=new Proxy(inner,{
                getOwnPropertyDescriptor:function(){
                    log.push("outer-descriptor");
                    return undefined;
                }
            });
            var descriptor=Object.getOwnPropertyDescriptor(outer,"x");
            return [descriptor===undefined,log.join("|")].join("|");
        })()
// case: pinned descriptor conversion replaces an abrupt HasProperty with the following Get
(function(){
            var first=true,log=[];
            var descriptor=new Proxy({
                value:42,writable:true,enumerable:true,configurable:true
            },{
                has:function(target,key){
                    log.push("has:"+key);
                    if(first){first=false;throw 17}
                    return Reflect.has(target,key);
                },
                get:function(target,key,receiver){
                    log.push("get:"+key);
                    return Reflect.get(target,key,receiver);
                }
            });
            var proxy=new Proxy({},{
                getOwnPropertyDescriptor:function(){return descriptor}
            });
            var result=Object.getOwnPropertyDescriptor(proxy,"x");
            return [__proxyBits(result),log.join("|")].join("|");
        })()
// REENTRANCY_CASES
// case: a get trap can reenter the same Proxy after changing its recursion state
(function(){
            var depth=0,log=[],target={x:41},handler={},proxy;
            handler.get=function(seen,key,receiver){
                log.push("get:"+depth+":"+(receiver===proxy));
                if(depth++===0)return Reflect.get(proxy,key,receiver)+1;
                return Reflect.get(seen,key,receiver);
            };
            proxy=new Proxy(target,handler);
            return [proxy.x,depth,log.join("|")].join("|");
        })()
// case: ownKeys can reenter getOwnPropertyDescriptor on the same Proxy
(function(){
            var log=[],target={x:42},handler={},proxy;
            handler.ownKeys=function(seen){
                log.push("keys");
                var descriptor=Reflect.getOwnPropertyDescriptor(proxy,"x");
                log.push("seen:"+descriptor.value);
                return Reflect.ownKeys(seen);
            };
            handler.getOwnPropertyDescriptor=function(seen,key){
                log.push("descriptor:"+key);
                return Reflect.getOwnPropertyDescriptor(seen,key);
            };
            proxy=new Proxy(target,handler);
            return __proxyKeys(Reflect.ownKeys(proxy))+"|"+log.join("|");
        })()
// case: a trap may revoke itself while target and handler stay live on the native stack
(function(){
            var pair,log=[],target={};
            Object.defineProperty(target,"x",{value:42,writable:false,configurable:false});
            pair=Proxy.revocable(target,{get:function(seen,key,receiver){
                log.push("trap:"+key+":"+(seen===target)+":"+(receiver===pair.proxy));
                pair.revoke();
                return 42;
            }});
            var first=pair.proxy.x;
            var second=__proxyCompletion(function(){return pair.proxy.x});
            return [first,second,log.join("|")].join("|");
        })()
// case: nested ownKeys fallback rechecks the Proxy which supplied the trap
(function(){
            var innerPair=Proxy.revocable({x:1},{
                ownKeys:function(){
                    innerPair.revoke();
                    return ["x"];
                }
            });
            var outer=new Proxy(innerPair.proxy,{});
            var first=__proxyCompletion(function(){return Reflect.ownKeys(outer)});

            var outerPair;
            var inner=new Proxy({x:1},{
                ownKeys:function(){
                    outerPair.revoke();
                    return ["x"];
                }
            });
            outerPair=Proxy.revocable(inner,{});
            var second=__proxyCompletion(function(){
                return __proxyKeys(Reflect.ownKeys(outerPair.proxy));
            });
            return [first,second].join("|");
        })()
