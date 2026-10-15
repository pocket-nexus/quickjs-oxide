// GRAPH_CASES
// case: global graph key order descriptors metadata identities and tag
(function(){
        function bit(value){return value?1:0}
        function bits(descriptor){
            return "D:"+bit(descriptor.writable)+bit(descriptor.enumerable)+bit(descriptor.configurable);
        }
        function isConstructor(value){
            try{Reflect.construct(function(){},[],value);return true}catch(_){return false}
        }
        function keyText(key){return typeof key==="symbol"?String(key):key}
        var names=["apply","construct","defineProperty","deleteProperty","get",
            "getOwnPropertyDescriptor","getPrototypeOf","has","isExtensible","ownKeys",
            "preventExtensions","set","setPrototypeOf"];
        var lengths=[3,2,3,2,2,2,1,2,1,1,1,3,2];
        var descriptor=Object.getOwnPropertyDescriptor(globalThis,"Reflect");
        var implemented=["String","Math","Reflect","Symbol","globalThis"];
        var metadata=[];
        for(var i=0;i<names.length;i++){
            var name=names[i],own=Object.getOwnPropertyDescriptor(Reflect,name),fn=own.value;
            var fnName=Object.getOwnPropertyDescriptor(fn,"name");
            var fnLength=Object.getOwnPropertyDescriptor(fn,"length");
            metadata[metadata.length]=name+":"+fn.name+":"+fn.length+":"+
                (fn.length===lengths[i])+":"+(Object.getPrototypeOf(fn)===Function.prototype)+":"+
                (typeof fn==="function")+":"+isConstructor(fn)+":"+bits(own)+":"+
                bits(fnName)+":"+bits(fnLength)+":"+Object.getOwnPropertyNames(fn).join(",");
        }
        var tag=Object.getOwnPropertyDescriptor(Reflect,Symbol.toStringTag);
        return [
            "global="+(descriptor.value===Reflect)+":"+bits(descriptor)+":"+
                (Object.getPrototypeOf(Reflect)===Object.prototype)+":"+Object.isExtensible(Reflect)+":"+
                Object.prototype.toString.call(Reflect),
            "global-order="+Reflect.ownKeys(globalThis).filter(function(key){
                return implemented.indexOf(key)>=0;
            }).join(","),
            "keys="+Reflect.ownKeys(Reflect).map(keyText).join(","),
            "methods="+metadata.join("|"),
            "identity="+(Reflect===Reflect)+":"+(Reflect.get===Reflect.get)+":"+
                (Reflect.get!==Reflect.set)+":"+(Reflect.construct!==Reflect.apply),
            "tag="+tag.value+":"+bits(tag)
        ].join("\n");
    })()
// AUTO_INIT_CASES
// case: fresh global AutoInit can be deleted without materializing or recovering
(function(){
            var keys=Object.getOwnPropertyNames(globalThis),present=keys.indexOf("Reflect")>=0;
            var deleted=delete globalThis.Reflect;
            return [present,deleted,"Reflect" in globalThis,typeof Reflect,
                Object.getOwnPropertyDescriptor(globalThis,"Reflect")===undefined].join(":");
        })()
// case: descriptor lookup materializes one stable configurable global object
(function(){
            var first=Object.getOwnPropertyDescriptor(globalThis,"Reflect"),value=first.value;
            var second=Object.getOwnPropertyDescriptor(globalThis,"Reflect");
            return [(value===Reflect),(second.value===value),(Reflect===Reflect),
                first.writable,first.enumerable,first.configurable,
                Object.getPrototypeOf(value)===Object.prototype].join(":");
        })()
// case: method mutation and deletion persist and deleting the global does not resurrect it
(function(){
            var saved=Reflect,originalGet=saved.get,marker=function marker(){return 17};
            saved.get=marker;
            var changed=saved.get===marker;
            var methodDelete=delete saved.construct;
            var methodMissing=!("construct" in saved)&&typeof saved.construct==="undefined";
            var globalDelete=delete globalThis.Reflect;
            var globalMissing=!("Reflect" in globalThis)&&typeof Reflect==="undefined";
            var detached=[saved.get===marker,saved.apply===saved.apply,
                !("construct" in saved),originalGet!==saved.get].join(":");
            globalThis.Reflect=saved;
            return [changed,methodDelete,methodMissing,globalDelete,globalMissing,detached,
                globalThis.Reflect===saved].join("|");
        })()
// case: defineProperty can replace a fresh global AutoInit and its flags stay exact
(function(){
            Object.defineProperty(globalThis,"Reflect",{
                value:17,writable:false,enumerable:true,configurable:true
            });
            var descriptor=Object.getOwnPropertyDescriptor(globalThis,"Reflect");
            return [Reflect,descriptor.value,descriptor.writable,descriptor.enumerable,
                descriptor.configurable,delete globalThis.Reflect,typeof Reflect].join(":");
        })()
// APPLY_CONSTRUCT_CASES
// case: apply preserves this and materializes inherited sparse array-like arguments
(function(){
            function target(a,b,c){
                return [this.marker,arguments.length,a,b,c,2 in arguments].join(":");
            }
            var prototype=Object();prototype[1]="B";
            var list=Object.create(prototype);list.length=3;list[0]="A";
            return Reflect.apply(target,{marker:"this"},list);
        })()
// case: apply reads length then indexes left to right and preserves abrupt getters
(function(){
            var log="",list=Object(),boom={token:"boom"};
            Object.defineProperty(list,"length",{get:function(){log+="l";return 3}});
            Object.defineProperty(list,"0",{get:function(){log+="0";return 20}});
            Object.defineProperty(list,"1",{get:function(){log+="1";return 22}});
            Object.defineProperty(list,"2",{get:function(){log+="2";throw boom}});
            var caught;
            try{Reflect.apply(function(){log+="b"},null,list)}catch(error){caught=error}
            list=Object();log+="|";
            Object.defineProperty(list,"length",{get:function(){log+="L";return 2}});
            Object.defineProperty(list,"0",{get:function(){log+="A";return 20}});
            Object.defineProperty(list,"1",{get:function(){log+="B";return 22}});
            var result=Reflect.apply(function(a,b){log+="C";return a+b},null,list);
            return [caught===boom,result,log].join(":");
        })()
// case: apply validates callable target before touching argsList and rejects primitive lists
(function(){
            var log="",poison=Object();
            Object.defineProperty(poison,"length",{get:function(){log+="length";throw "poison"}});
            var invalidTarget,primitiveList,missingList;
            try{Reflect.apply({},null,poison)}catch(error){invalidTarget=error.name}
            var afterTarget=log;
            try{Reflect.apply(function(){},null,"ab")}catch(error){primitiveList=error.name}
            try{Reflect.apply(function(){},null)}catch(error){missingList=error.name}
            return [invalidTarget,afterTarget,primitiveList,missingList,log].join(":");
        })()
// case: construct supplies new.target and supports subclass-style custom prototypes
(function(){
            function Base(a,b){this.sum=a+b;this.seen=new.target}
            function Derived(){}
            var prototype={kind:"derived"};Derived.prototype=prototype;
            var value=Reflect.construct(Base,[20,22],Derived);
            return [value.sum,value.seen===Derived,Object.getPrototypeOf(value)===prototype,
                value instanceof Derived,value instanceof Base,value.kind].join(":");
        })()
// case: construct honors object returns and ignores primitive constructor returns
(function(){
            var override={answer:42};
            function ReturnsObject(){this.local=1;return override}
            function ReturnsPrimitive(){this.answer=42;return 17}
            var first=Reflect.construct(ReturnsObject,[]),second=Reflect.construct(ReturnsPrimitive,[]);
            return [first===override,first.local,second.answer,
                Object.getPrototypeOf(second)===ReturnsPrimitive.prototype].join(":");
        })()
// case: construct pins QuickJS newTarget argsList and target validation order
(function(){
            var log="",poison=Object(),argumentError={kind:"args"};
            Object.defineProperty(poison,"length",{get:function(){log+="L";throw argumentError}});
            var bothInvalid,newTargetFirst,argsBeforeTarget,targetAfterList;
            try{Reflect.construct({},poison,null)}catch(error){bothInvalid=error.name}
            var afterBoth=log;
            try{Reflect.construct(function(){},poison,{})}catch(error){newTargetFirst=error.name}
            var afterNewTarget=log;
            try{Reflect.construct({},poison)}catch(error){argsBeforeTarget=error===argumentError}
            var afterArgs=log;
            try{Reflect.construct({},[])}catch(error){targetAfterList=error.name}
            return [bothInvalid,afterBoth,newTargetFirst,afterNewTarget,argsBeforeTarget,
                afterArgs,targetAfterList].join(":");
        })()
// case: construct extracts arguments before newTarget prototype and constructor body
(function(){
            var log="",list=Object(),prototype={realm:"custom"};
            Object.defineProperty(list,"length",{get:function(){log+="l";return 2}});
            Object.defineProperty(list,"0",{get:function(){log+="0";return 20}});
            Object.defineProperty(list,"1",{get:function(){log+="1";return 22}});
            function Target(a,b){log+="b";this.answer=a+b}
            var NewTarget=Target.bind(null);
            Object.defineProperty(NewTarget,"prototype",{
                get:function(){log+="p";return prototype},configurable:true
            });
            var value=Reflect.construct(Target,list,NewTarget);
            return [value.answer,Object.getPrototypeOf(value)===prototype,log].join(":");
        })()
// case: construct rejects primitive lists and defaults newTarget to target
(function(){
            function Target(a){this.value=a;this.same=new.target===Target}
            var primitive,missing;
            try{Reflect.construct(Target,"x")}catch(error){primitive=error.name}
            try{Reflect.construct(Target)}catch(error){missing=error.name}
            var value=Reflect.construct(Target,{0:42,length:1});
            return [primitive,missing,value.value,value.same,
                Object.getPrototypeOf(value)===Target.prototype].join(":");
        })()
// PROPERTY_CASES
// case: defineProperty converts key then descriptor fields and returns booleans
(function(){
            var log="",target=Object(),key=Object(),descriptor=Object();
            key[Symbol.toPrimitive]=function(hint){log+="k"+hint+",";return "answer"};
            Object.defineProperty(descriptor,"enumerable",{get:function(){log+="e,";return true}});
            Object.defineProperty(descriptor,"configurable",{get:function(){log+="c,";return true}});
            Object.defineProperty(descriptor,"value",{get:function(){log+="v,";return 42}});
            Object.defineProperty(descriptor,"writable",{get:function(){log+="w,";return false}});
            var created=Reflect.defineProperty(target,key,descriptor),own=Object.getOwnPropertyDescriptor(target,"answer");
            Object.preventExtensions(target);
            var newFailed=Reflect.defineProperty(target,"late",{value:1});
            var incompatible=Reflect.defineProperty(target,"answer",{writable:true});
            return [created,log,own.value,own.writable,own.enumerable,own.configurable,
                newFailed,incompatible,target.hasOwnProperty("late")].join(":");
        })()
// case: defineProperty validates target before key and descriptor and preserves abrupt conversion
(function(){
            var log="",key=Object(),descriptor=Object(),boom={kind:"descriptor"};
            key[Symbol.toPrimitive]=function(){log+="k";throw "key"};
            Object.defineProperty(descriptor,"enumerable",{get:function(){log+="d";throw boom}});
            var invalid,keyThrow,descriptorThrow,target=Object();
            try{Reflect.defineProperty(null,key,descriptor)}catch(error){invalid=error.name}
            var afterInvalid=log;
            try{Reflect.defineProperty(target,key,descriptor)}catch(error){keyThrow=error}
            key[Symbol.toPrimitive]=function(){log+="K";return "x"};
            try{Reflect.defineProperty(target,key,descriptor)}catch(error){descriptorThrow=error===boom}
            return [invalid,afterInvalid,keyThrow,descriptorThrow,log].join(":");
        })()
// case: deleteProperty handles strings symbols missing and nonconfigurable properties
(function(){
            var target=Object(),symbol=Symbol("delete");target.open=1;target[symbol]=2;
            Object.defineProperty(target,"fixed",{value:3,configurable:false});
            var a=Reflect.deleteProperty(target,"open"),b=Reflect.deleteProperty(target,symbol);
            var c=Reflect.deleteProperty(target,"missing"),d=Reflect.deleteProperty(target,"fixed");
            return [a,b,c,d,target.hasOwnProperty("open"),target.hasOwnProperty(symbol),
                target.fixed].join(":");
        })()
// case: get and has use property keys prototypes accessors and explicit receivers
(function(){
            var log="",symbol=Symbol("key"),prototype=Object(),target=Object.create(prototype);
            prototype.inherited=20;target[symbol]=22;
            Object.defineProperty(prototype,"access",{get:function(){log+=this.marker;return this.value}});
            var receiver={marker:"r",value:42};
            var key=Object();key[Symbol.toPrimitive]=function(hint){log+=hint;return "inherited"};
            return [Reflect.get(target,"inherited"),Reflect.get(target,symbol),
                Reflect.get(target,"access",receiver),Reflect.get(target,key),
                Reflect.has(target,"inherited"),Reflect.has(target,symbol),
                Reflect.has(target,"missing"),log].join(":");
        })()
// case: getOwnPropertyDescriptor returns fresh exact descriptors for string and symbol keys
(function(){
            var target=Object(),payload=Object(),symbol=Symbol("own"),calls=0;
            Object.defineProperty(target,"data",{value:payload,writable:false,enumerable:true,configurable:false});
            var getter=function(){calls++;return 7};
            Object.defineProperty(target,symbol,{get:getter,set:undefined,enumerable:false,configurable:true});
            var first=Reflect.getOwnPropertyDescriptor(target,"data"),second=Reflect.getOwnPropertyDescriptor(target,"data");
            var access=Reflect.getOwnPropertyDescriptor(target,symbol);
            return [Object.getOwnPropertyNames(first).join(","),first.value===payload,first.writable,
                first.enumerable,first.configurable,first!==second,
                Object.getOwnPropertyNames(access).join(","),access.get===getter,
                access.set===undefined,access.enumerable,access.configurable,calls,
                Reflect.getOwnPropertyDescriptor(target,"missing")===undefined].join(":");
        })()
// case: getPrototypeOf and setPrototypeOf preserve null and report failed changes
(function(){
            var first=Object(),second=Object(),target=Object.create(first);
            var changed=Reflect.setPrototypeOf(target,second),after=Reflect.getPrototypeOf(target)===second;
            var toNull=Reflect.setPrototypeOf(target,null),nullProto=Reflect.getPrototypeOf(target)===null;
            var restored=Reflect.setPrototypeOf(target,first);Object.preventExtensions(target);
            var same=Reflect.setPrototypeOf(target,first),failed=Reflect.setPrototypeOf(target,second);
            var badPrototype;
            try{Reflect.setPrototypeOf(target,17)}catch(error){badPrototype=error.name}
            return [changed,after,toNull,nullProto,restored,same,failed,
                Reflect.getPrototypeOf(target)===first,badPrototype].join(":");
        })()
// case: isExtensible and preventExtensions require objects and return booleans
(function(){
            var target=Object(),before=Reflect.isExtensible(target),prevented=Reflect.preventExtensions(target);
            var after=Reflect.isExtensible(target),again=Reflect.preventExtensions(target);
            var isPrimitive,preventPrimitive;
            try{Reflect.isExtensible(1)}catch(error){isPrimitive=error.name}
            try{Reflect.preventExtensions(null)}catch(error){preventPrimitive=error.name}
            return [before,prevented,after,again,isPrimitive,preventPrimitive].join(":");
        })()
// case: ownKeys returns canonical own string and symbol order including nonenumerables
(function(){
            var first=Symbol("first"),second=Symbol("second"),prototype={inherited:1};
            var target=Object.create(prototype);target.z=1;target[10]=2;target[first]=3;target[2]=4;target.a=5;
            Object.defineProperty(target,"hidden",{value:6,enumerable:false});target[second]=7;
            var keys=Reflect.ownKeys(target),out=[];
            for(var i=0;i<keys.length;i++)out[i]=typeof keys[i]==="symbol"?
                (keys[i]===first?"first":"second"):keys[i];
            return [out.join(","),Array.isArray(keys),Object.getPrototypeOf(keys)===Array.prototype,
                keys.indexOf("inherited")<0].join(":");
        })()
// case: set uses receiver accessors symbols and returns false for rejected writes
(function(){
            var log="",symbol=Symbol("set"),prototype=Object(),target=Object.create(prototype);
            var receiver={marker:"R"};
            Object.defineProperty(prototype,"access",{set:function(value){log+=this.marker+value}});
            Object.defineProperty(prototype,"readonly",{value:1,writable:false});
            Object.defineProperty(prototype,"data",{value:2,writable:true});
            var accessor=Reflect.set(target,"access",42,receiver);
            var inheritedData=Reflect.set(target,"data",20,receiver);
            var readOnly=Reflect.set(target,"readonly",9,receiver);
            var symbolSet=Reflect.set(target,symbol,22);
            Object.preventExtensions(target);
            var late=Reflect.set(target,"late",1);
            return [accessor,log,inheritedData,receiver.data,target.hasOwnProperty("data"),
                readOnly,receiver.hasOwnProperty("readonly"),symbolSet,target[symbol],late,
                target.hasOwnProperty("late")].join(":");
        })()
// case: keyed methods validate target before converting property keys
(function(){
            var names=["deleteProperty","get","getOwnPropertyDescriptor","has","set"],out=[],log="";
            for(var i=0;i<names.length;i++){
                var key=Object();key[Symbol.toPrimitive]=function(){log+="k";throw "key"};
                try{Reflect[names[i]](null,key,1)}catch(error){out[out.length]=error.name+":"+log}
            }
            return out.join("|");
        })()
