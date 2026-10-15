// PRIMITIVE_CASES
// case: primitive identity including nullish Symbol BigInt minus zero and NaN
(function(){
            function sameValue(a,b){return a===b?(a!==0||1/a===1/b):(a!==a&&b!==b)}
            var symbol=Symbol("identity"),bigint=9007199254740993n,minusZero=-0,nan=NaN;
            var values=[undefined,null,false,17,"oxide",bigint,symbol,minusZero,nan],out=[];
            for(var i=0;i<values.length;i++){
                var value=values[i],returned=Object.preventExtensions(value);
                out[out.length]=typeof value+":"+sameValue(returned,value)+":"+Object.isExtensible(value);
            }
            return out.join("|")+"|missing:"+sameValue(Object.preventExtensions(),undefined)+":"+Object.isExtensible();
        })()
// case: primitive calls do not box or consult wrapper prototypes
(function(){
            Number.prototype.marker=1;Boolean.prototype.marker=2;
            BigInt.prototype.marker=4;Symbol.prototype.marker=5;
            var symbol=Symbol("plain");
            return [Object.preventExtensions(3)===3,Object.preventExtensions(true)===true,
                Object.preventExtensions("x")==="x",Object.preventExtensions(8n)===8n,
                Object.preventExtensions(symbol)===symbol,
                Object.isExtensible(3),Object.isExtensible(true),Object.isExtensible("x"),
                Object.isExtensible(8n),Object.isExtensible(symbol)].join(":");
        })()
// OBJECT_CASES
// case: ordinary Array String wrapper Function and Error objects share the operation
(function(){
            function sample(){}
            var values=[Object(),[1,2],Object("ab"),sample,new Error("oxide")],labels=["object","array","string","function","error"],out=[];
            for(var i=0;i<values.length;i++){
                var value=values[i],before=Object.isExtensible(value);
                var first=Object.preventExtensions(value),second=Object.preventExtensions(value);
                out[out.length]=labels[i]+":"+before+":"+(first===value)+":"+(second===value)+":"+Object.isExtensible(value);
            }
            return out.join("|")+"|payload:"+values[1].join(",")+":"+values[2][0]+values[2][1]+":"+
                (values[3]===sample)+":"+values[4].message;
        })()
// case: prevention is identity preserving and idempotent across aliases
(function(){
            var value=Object(),alias=value;value.x=1;
            var a=Object.preventExtensions(value),b=Object.preventExtensions(alias),c=Object.preventExtensions(a);
            return (a===value)+":"+(b===value)+":"+(c===value)+":"+(a===b)+":"+
                Object.isExtensible(value)+":"+Object.isExtensible(alias)+":"+Object.keys(value).join(",");
        })()
// case: non-extensible Object materializes an existing AutoInit and global state persists
(function(){
            var prevent=Object.preventExtensions,objectConstructor=Object;
            var sameObject=prevent(objectConstructor)===objectConstructor;
            var isExtensible=Object.isExtensible;
            var lazyMaterialized=typeof isExtensible==="function";
            var objectState=isExtensible(objectConstructor);
            globalThis.extensibilityExisting=1;
            var sameGlobal=prevent(globalThis)===globalThis;
            globalThis.extensibilityExisting=2;
            var strictAdd;
            try{(function(){"use strict";globalThis.extensibilityLate=3})()}
            catch(error){strictAdd=error.name+":"+error.message}
            return [sameObject,lazyMaterialized,objectState,sameGlobal,
                isExtensible(globalThis),globalThis.extensibilityExisting,
                globalThis.hasOwnProperty("extensibilityLate")].join(":")+"|"+strictAdd;
        })()
// MUTATION_CASES
// case: existing properties update and delete while additions and prototype changes reject
(function(){
            var firstProto=Object(),secondProto=Object();firstProto.first=true;secondProto.second=true;
            var object=Object.create(firstProto);
            var keptDescriptor=Object();keptDescriptor.value=1;keptDescriptor.writable=true;keptDescriptor.enumerable=true;keptDescriptor.configurable=true;
            var removedDescriptor=Object();removedDescriptor.value=2;removedDescriptor.writable=true;removedDescriptor.enumerable=true;removedDescriptor.configurable=true;
            Object.defineProperty(object,"kept",keptDescriptor);
            Object.defineProperty(object,"removed",removedDescriptor);
            Object.preventExtensions(object);
            object.kept=3;
            var deleted=delete object.removed;
            object.sloppy=4;
            var strictAdd,defineAdd,differentPrototype;
            try{(function(){"use strict";object.strict=5})()}catch(error){strictAdd=error.name+":"+error.message}
            var addedDescriptor=Object();addedDescriptor.value=6;
            try{Object.defineProperty(object,"defined",addedDescriptor)}catch(error){defineAdd=error.name+":"+error.message}
            var samePrototype=Object.setPrototypeOf(object,firstProto)===object;
            try{Object.setPrototypeOf(object,secondProto)}catch(error){differentPrototype=error.name+":"+error.message}
            var updateDescriptor=Object();updateDescriptor.value=7;Object.defineProperty(object,"kept",updateDescriptor);
            return Object.isExtensible(object)+":"+object.kept+":"+deleted+":"+
                object.hasOwnProperty("removed")+":"+object.hasOwnProperty("sloppy")+":"+
                object.hasOwnProperty("strict")+":"+object.hasOwnProperty("defined")+":"+
                samePrototype+":"+(Object.getPrototypeOf(object)===firstProto)+"|"+
                strictAdd+"|"+defineAdd+"|"+differentPrototype;
        })()
// case: Array and String exotic existing surfaces survive prevention
(function(){
            var array=["a","b"],string=Object("xy");
            array.extra=1;string.extra=2;
            Object.preventExtensions(array);Object.preventExtensions(string);
            array[0]="A";array.extra=3;string.extra=4;
            var arrayDelete=delete array[1],stringDelete=delete string.extra;
            array[2]="C";string.late="L";
            return Object.isExtensible(array)+":"+array.length+":"+array[0]+":"+arrayDelete+":"+
                array.hasOwnProperty(1)+":"+array.hasOwnProperty(2)+":"+array.extra+"|"+
                Object.isExtensible(string)+":"+string[0]+string[1]+":"+stringDelete+":"+
                string.hasOwnProperty("extra")+":"+string.hasOwnProperty("late");
        })()
// EXOTIC_CASES
// case: Proxy isExtensible forwards the trap and preserves its order
(function(){
            var log="",base=Object();
            var proxy=new Proxy(base,{isExtensible:function(target){
                log+="is|";return Reflect.isExtensible(target);
            }});
            return Object.isExtensible(proxy)+"|"+log;
        })()
// case: Proxy preventExtensions forwards the trap and returns the proxy
(function(){
            var log="",base=Object();
            var proxy=new Proxy(base,{preventExtensions:function(target){
                log+="prevent|";return Reflect.preventExtensions(target);
            }});
            return (Object.preventExtensions(proxy)===proxy)+"|"+
                Object.isExtensible(base)+"|"+log;
        })()
// case: Proxy isExtensible invariant rejects a result inconsistent with its target
(function(){
            var log="",base=Object();
            var proxy=new Proxy(base,{isExtensible:function(){
                log+="is|";return false;
            }});
            try{Object.isExtensible(proxy)}catch(error){
                return log+error.name+":"+error.message;
            }
            return "missing";
        })()
// case: Resizable ArrayBuffer backed TypedArray rejects preventExtensions
(function(){
            var buffer=new ArrayBuffer(4,{maxByteLength:8});
            var value=new Uint8Array(buffer);
            try{Object.preventExtensions(value)}catch(error){
                return "prevent-throw|"+error.name+"|"+error.message+"|"+
                    Object.isExtensible(value);
            }
            return "missing";
        })()
