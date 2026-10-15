// OWN_CASES
// case: ordinary own data accessor non-enumerable and Symbol properties
(function(){
            var hits=0,proto=Object(),object=Object.create(proto),hidden=Object(),accessor=Object();
            var ownSymbol=Symbol("own"),inheritedSymbol=Symbol("inherited");
            proto.inherited=1;proto[inheritedSymbol]=2;object.visible=3;object[ownSymbol]=4;
            hidden.value=5;hidden.writable=false;hidden.enumerable=false;hidden.configurable=true;
            Object.defineProperty(object,"hidden",hidden);
            accessor.enumerable=true;accessor.configurable=true;
            accessor.get=function(){hits++;throw "getter"};
            Object.defineProperty(object,"accessor",accessor);
            return [
                Object.hasOwn.call(null,object,"visible"),Object.hasOwn(object,"hidden"),
                Object.hasOwn(object,"accessor"),hits,Object.hasOwn(object,"inherited"),
                Object.hasOwn(object,ownSymbol),Object.hasOwn(object,inheritedSymbol),
                Object.hasOwn(object,"missing")
            ].join(":");
        })()
// case: primitive property keys use ToPropertyKey while Symbols retain identity
(function(){
            var object=Object(),symbol=Symbol("key"),other=Symbol("key");
            object[0]="zero";object.NaN="nan";object.true="boolean";
            object.null="null";object.undefined="undefined";object[1]="bigint";object[symbol]="symbol";
            return [
                Object.hasOwn(object,-0),Object.hasOwn(object,NaN),Object.hasOwn(object,true),
                Object.hasOwn(object,null),Object.hasOwn(object,undefined),Object.hasOwn(object,1n),
                Object.hasOwn(object,symbol),Object.hasOwn(object,other)
            ].join(":");
        })()
// case: primitive targets are boxed and String virtual indices are own
(function(){
            var symbol=Symbol("target");
            return [
                Object.hasOwn("𝄞a",0),Object.hasOwn("𝄞a",1),Object.hasOwn("𝄞a",2),
                Object.hasOwn("𝄞a",3),Object.hasOwn("𝄞a","length"),
                Object.hasOwn(17,"valueOf"),Object.hasOwn(false,"valueOf"),
                Object.hasOwn(1n,"valueOf"),Object.hasOwn(symbol,"description")
            ].join(":");
        })()
// case: Array holes length and function lazy properties retain ownness
(function(){
            var array=[];array[2]=7;
            function ordinary(){}
            var bound=ordinary.bind(null);
            return [
                Object.hasOwn(array,0),Object.hasOwn(array,1),Object.hasOwn(array,2),
                Object.hasOwn(array,"length"),Object.hasOwn(ordinary,"length"),
                Object.hasOwn(ordinary,"name"),Object.hasOwn(ordinary,"prototype"),
                Object.hasOwn(bound,"length"),Object.hasOwn(bound,"name"),
                Object.hasOwn(bound,"prototype")
            ].join(":");
        })()
// case: ownness tracks descriptor creation deletion and prototype shadowing
(function(){
            var proto=Object(),object=Object.create(proto),descriptor=Object();proto.x=1;
            var before=Object.hasOwn(object,"x");
            descriptor.value=2;descriptor.writable=false;descriptor.enumerable=false;descriptor.configurable=true;
            Object.defineProperty(object,"x",descriptor);
            var defined=Object.hasOwn(object,"x"),deleted=delete object.x;
            return before+":"+defined+":"+deleted+":"+Object.hasOwn(object,"x")+":"+(object.x===1);
        })()
// ORDER_CASES
// case: nullish target rejection precedes every property-key hook
(function(){
            function run(target){
                var log="",key=Object();
                key[Symbol.toPrimitive]=function(hint){log+="key:"+hint+";";return "x"};
                try{Object.hasOwn(target,key)}catch(error){return log+error.name+":"+error.message}
                return "missing";
            }
            return run(null)+"|"+run(undefined);
        })()
// case: target objects are not converted and key conversion completes before lookup
(function(){
            var log="",target=Object(),key=Object();target.old=1;
            target.__defineGetter__(Symbol.toPrimitive,function(){log+="target-hook;";throw "target"});
            key[Symbol.toPrimitive]=function(hint){
                log+="key:"+hint+";";delete target.old;target.late=2;return "late";
            };
            var result=Object.hasOwn(target,key);
            return log+"|"+result+":"+Object.hasOwn(target,"old")+":"+Object.hasOwn(target,"late");
        })()
// case: ordinary key conversion falls back from toString to valueOf
(function(){
            var log="",target=Object(),key=Object();target.x=1;key[Symbol.toPrimitive]=undefined;
            key.__defineGetter__("toString",function(){
                log+="get-toString;";return function(){log+="call-toString;";return Object()};
            });
            key.__defineGetter__("valueOf",function(){
                log+="get-valueOf;";return function(){log+="call-valueOf;";return "x"};
            });
            return Object.hasOwn(target,key)+"|"+log;
        })()
// case: own descriptor lookup does not read accessor values
(function(){
            var calls=0,target=Object(),descriptor=Object();
            descriptor.enumerable=true;descriptor.configurable=true;
            descriptor.get=function(){calls++;throw "get"};
            Object.defineProperty(target,"x",descriptor);
            return Object.hasOwn(target,"x")+":"+calls;
        })()
// ERROR_CASES
// case: missing target
Object.hasOwn()
// case: undefined target
Object.hasOwn(undefined,'x')
// case: null target
Object.hasOwn(null,'x')
// case: property-key conversion preserves a primitive throw
(function(){var key=Object();key[Symbol.toPrimitive]=function(){throw "key"};return Object.hasOwn(Object(),key)})()
// case: property-key conversion rejects an object result
(function(){var key=Object();key[Symbol.toPrimitive]=function(){return Object()};return Object.hasOwn(Object(),key)})()
// case: property-key conversion rejects a non-callable exotic hook
(function(){var key=Object();key[Symbol.toPrimitive]=1;return Object.hasOwn(Object(),key)})()
// case: hasOwn is not a constructor
new Object.hasOwn(Object(),'x')
// EXOTIC_CASES
// case: Proxy own lookup follows key conversion and only invokes the descriptor trap
(function(){
            var log="",base=Object(),key=Object();base.x=1;
            key[Symbol.toPrimitive]=function(hint){log+="key:"+hint+";";return "x"};
            var proxy=new Proxy(base,{
                getOwnPropertyDescriptor:function(target,name){
                    log+="descriptor:"+String(name)+";";return Reflect.getOwnPropertyDescriptor(target,name);
                },
                get:function(){log+="get;";throw "get"}
            });
            return Object.hasOwn(proxy,key)+"|"+log;
        })()
// case: Proxy descriptor invariant failures remain observable
(function(){
            var log="",base=Object.preventExtensions(Object());
            var proxy=new Proxy(base,{getOwnPropertyDescriptor:function(){
                log+="descriptor;";
                return {value:1,writable:true,enumerable:true,configurable:true};
            }});
            try{Object.hasOwn(proxy,"x")}catch(error){return log+error.name+":"+error.message}
            return "missing";
        })()
// case: TypedArray integer-indexed elements are own but length is inherited
(function(){
            var value=new Uint8Array([7,8]);
            return [Object.hasOwn(value,0),Object.hasOwn(value,1),Object.hasOwn(value,2),
                Object.hasOwn(value,"-0"),Object.hasOwn(value,"length")].join(":");
        })()
// case: mapped arguments expose and can delete indexed own properties
(function(value){
            var before=Object.hasOwn(arguments,0),length=Object.hasOwn(arguments,"length");
            var callee=Object.hasOwn(arguments,"callee"),deleted=delete arguments[0];
            return before+":"+length+":"+callee+":"+deleted+":"+Object.hasOwn(arguments,0)+":"+value;
        })(7)
