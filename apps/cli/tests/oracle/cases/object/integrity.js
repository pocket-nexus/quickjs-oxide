// PRIMITIVE_CASES
// case: seal and freeze preserve every primitive without boxing
(function(){
            var symbol=Symbol("integrity");
            return [
                Object.seal()===undefined,Object.freeze()===undefined,
                Object.seal(null)===null,Object.freeze(null)===null,
                Object.seal(false)===false,Object.freeze(true)===true,
                Object.seal(17)===17,Object.freeze(19)===19,
                Object.is(Object.seal(-0),-0),Object.is(Object.freeze(NaN),NaN),
                Object.seal("ab")==="ab",Object.freeze("cd")==="cd",
                Object.seal(23n)===23n,Object.freeze(29n)===29n,
                Object.seal(symbol)===symbol,Object.freeze(symbol)===symbol
            ].join(":");
        })()
// case: integrity predicates report every primitive including missing arguments true
(function(){
            var symbol=Symbol("query");
            return [
                Object.isSealed(),Object.isFrozen(),
                Object.isSealed(undefined),Object.isFrozen(undefined),
                Object.isSealed(null),Object.isFrozen(null),
                Object.isSealed(false),Object.isFrozen(true),
                Object.isSealed(0),Object.isFrozen(-0),
                Object.isSealed("x"),Object.isFrozen("x"),
                Object.isSealed(1n),Object.isFrozen(1n),
                Object.isSealed(symbol),Object.isFrozen(symbol)
            ].join(":");
        })()
// DESCRIPTOR_CASES
// case: seal preserves data writability and accessor identity while clearing configurability
(function(){
            function dataBits(descriptor){return descriptor.value+":"+descriptor.writable+":"+descriptor.enumerable+":"+descriptor.configurable}
            var calls=0,target=Object(),data=Object(),accessor=Object();
            function getter(){calls++;throw "getter"}function setter(value){calls+=value}
            data.value=17;data.writable=true;data.enumerable=true;data.configurable=true;
            accessor.get=getter;accessor.set=setter;accessor.enumerable=false;accessor.configurable=true;
            Object.defineProperty(target,"data",data);Object.defineProperty(target,"accessor",accessor);
            var returned=Object.seal(target),dataAfter=Object.getOwnPropertyDescriptor(target,"data");
            var accessorAfter=Object.getOwnPropertyDescriptor(target,"accessor");
            return (returned===target)+"|"+dataBits(dataAfter)+"|"+
                (accessorAfter.get===getter)+":"+(accessorAfter.set===setter)+":"+
                accessorAfter.enumerable+":"+accessorAfter.configurable+":"+calls+"|"+
                Object.isSealed(target)+":"+Object.isFrozen(target)+":"+Object.isExtensible(target);
        })()
// case: freeze preserves values and accessor identity while making data non writable
(function(){
            var calls=0,payload=Object(),target=Object(),data=Object(),locked=Object(),accessor=Object();
            function getter(){calls++;throw "getter"}function setter(value){calls+=value}
            data.value=payload;data.writable=true;data.enumerable=true;data.configurable=true;
            locked.value=31;locked.writable=false;locked.enumerable=false;locked.configurable=true;
            accessor.get=getter;accessor.set=setter;accessor.enumerable=true;accessor.configurable=true;
            Object.defineProperty(target,"data",data);Object.defineProperty(target,"locked",locked);
            Object.defineProperty(target,"accessor",accessor);
            var returned=Object.freeze(target),d=Object.getOwnPropertyDescriptor(target,"data");
            var l=Object.getOwnPropertyDescriptor(target,"locked"),a=Object.getOwnPropertyDescriptor(target,"accessor");
            return (returned===target)+"|"+(d.value===payload)+":"+d.writable+":"+d.enumerable+":"+d.configurable+"|"+
                l.value+":"+l.writable+":"+l.enumerable+":"+l.configurable+"|"+
                (a.get===getter)+":"+(a.set===setter)+":"+a.enumerable+":"+a.configurable+":"+calls+"|"+
                Object.isSealed(target)+":"+Object.isFrozen(target)+":"+Object.isExtensible(target);
        })()
// case: nonenumerable and Symbol own keys are tightened while inherited properties are untouched
(function(){
            var proto=Object(),target=Object.create(proto),hidden=Object(),symbol=Symbol("own");
            proto.inherited=1;target[symbol]=2;
            hidden.value=3;hidden.writable=true;hidden.enumerable=false;hidden.configurable=true;
            Object.defineProperty(target,"hidden",hidden);Object.freeze(target);
            var ownSymbol=Object.getOwnPropertyDescriptor(target,symbol);
            var ownHidden=Object.getOwnPropertyDescriptor(target,"hidden");
            var inherited=Object.getOwnPropertyDescriptor(proto,"inherited");
            return ownSymbol.value+":"+ownSymbol.writable+":"+ownSymbol.enumerable+":"+ownSymbol.configurable+"|"+
                ownHidden.value+":"+ownHidden.writable+":"+ownHidden.enumerable+":"+ownHidden.configurable+"|"+
                inherited.writable+":"+inherited.enumerable+":"+inherited.configurable+"|"+
                (Object.getOwnPropertyDescriptor(target,"inherited")===undefined);
        })()
// case: Array and String exotic own properties receive their exact integrity flags
(function(){
            function bits(object,key){var d=Object.getOwnPropertyDescriptor(object,key);return d.writable+":"+d.enumerable+":"+d.configurable}
            var sealed=[];sealed[0]="A";sealed[2]="C";sealed.extra="E";Object.seal(sealed);
            var frozen=[];frozen[0]="F";frozen.extra="X";Object.freeze(frozen);
            var string=Object("ab");string.extra="S";Object.freeze(string);
            return Object.getOwnPropertyNames(sealed).join(",")+"|"+
                bits(sealed,"0")+":"+bits(sealed,"2")+":"+bits(sealed,"length")+":"+bits(sealed,"extra")+"|"+
                bits(frozen,"0")+":"+bits(frozen,"length")+":"+bits(frozen,"extra")+"|"+
                Object.getOwnPropertyNames(string).join(",")+"|"+
                bits(string,"0")+":"+bits(string,"1")+":"+bits(string,"length")+":"+bits(string,"extra")+"|"+
                Object.isSealed(sealed)+":"+Object.isFrozen(sealed)+":"+
                Object.isSealed(frozen)+":"+Object.isFrozen(frozen)+":"+Object.isFrozen(string);
        })()
// MUTATION_CASES
// case: seal then freeze is monotonic idempotent and preserves allowed writes
(function(){
            var object=Object();object.value=1;
            var before=Object.isExtensible(object)+":"+Object.isSealed(object)+":"+Object.isFrozen(object);
            var firstSeal=Object.seal(object),secondSeal=Object.seal(object);
            object.value=2;object.added=3;var deleted=delete object.value;
            var afterSeal=Object.isExtensible(object)+":"+Object.isSealed(object)+":"+Object.isFrozen(object)+":"+
                object.value+":"+(object.added===undefined)+":"+deleted;
            var firstFreeze=Object.freeze(object),secondFreeze=Object.freeze(object);object.value=4;
            var descriptor=Object.getOwnPropertyDescriptor(object,"value");
            return before+"|"+(firstSeal===object)+":"+(secondSeal===object)+"|"+afterSeal+"|"+
                (firstFreeze===object)+":"+(secondFreeze===object)+"|"+
                object.value+":"+descriptor.writable+":"+descriptor.configurable+":"+
                Object.isSealed(object)+":"+Object.isFrozen(object);
        })()
// case: preventExtensions alone exposes all four predicate combinations allowed for ordinary data
(function(){
            function row(value){return Object.isSealed(value)+":"+Object.isFrozen(value)}
            var empty=Object();Object.preventExtensions(empty);
            var configurable=Object();configurable.x=1;Object.preventExtensions(configurable);
            var writable=Object(),wd=Object();wd.value=2;wd.writable=true;wd.enumerable=true;wd.configurable=false;
            Object.defineProperty(writable,"x",wd);Object.preventExtensions(writable);
            var frozen=Object(),fd=Object();fd.value=3;fd.writable=false;fd.enumerable=true;fd.configurable=false;
            Object.defineProperty(frozen,"x",fd);Object.preventExtensions(frozen);
            var accessor=Object(),ad=Object();ad.get=function(){throw "unused"};ad.enumerable=true;ad.configurable=false;
            Object.defineProperty(accessor,"x",ad);Object.preventExtensions(accessor);
            return row(empty)+"|"+row(configurable)+"|"+row(writable)+"|"+row(frozen)+"|"+row(accessor);
        })()
// case: sealed Array elements remain writable but additions and deletion are rejected
(function(){
            var array=[];array[0]="A";array[2]="C";Object.seal(array);
            array[0]="B";array[1]="new";array[3]="late";var removed=delete array[2];
            var beforeFreeze=array[0]+":"+(array[1]===undefined)+":"+(array[3]===undefined)+":"+removed+":"+array.length;
            Object.freeze(array);array[0]="D";
            return beforeFreeze+"|"+array[0]+":"+Object.getOwnPropertyDescriptor(array,"length").writable+":"+
                Object.isSealed(array)+":"+Object.isFrozen(array);
        })()
// case: integrity operations and predicates never execute stored accessors
(function(){
            var calls=0,sealed=Object(),frozen=Object();
            sealed.__defineGetter__("x",function(){calls++;throw "sealed getter"});
            frozen.__defineGetter__("x",function(){calls++;throw "frozen getter"});
            var a=Object.seal(sealed),b=Object.isSealed(sealed),c=Object.isFrozen(sealed);
            var d=Object.freeze(frozen),e=Object.isSealed(frozen),f=Object.isFrozen(frozen);
            return (a===sealed)+":"+b+":"+c+":"+(d===frozen)+":"+e+":"+f+":"+calls;
        })()
// EXOTIC_CASES
// case: Proxy defineProperty failure leaves preventExtensions and earlier keys applied
(function(){
            var log="",base=Object(),a=Object(),b=Object();
            a.value=1;a.writable=true;a.enumerable=true;a.configurable=true;
            b.value=2;b.writable=true;b.enumerable=true;b.configurable=true;
            Object.defineProperty(base,"a",a);Object.defineProperty(base,"b",b);
            var handler={
                preventExtensions:function(target){log+="prevent|";return Reflect.preventExtensions(target)},
                ownKeys:function(){log+="keys|";return ["a","b"]},
                defineProperty:function(target,key,descriptor){log+="define-"+key+"|";if(key==="b")return false;return Reflect.defineProperty(target,key,descriptor)}
            };
            var proxy=new Proxy(base,handler);
            try{Object.seal(proxy)}catch(error){
                return error.name+"|"+log+"|"+Object.isExtensible(base)+":"+
                    Object.getOwnPropertyDescriptor(base,"a").configurable+":"+
                    Object.getOwnPropertyDescriptor(base,"b").configurable;
            }
            return "missing";
        })()
// case: freezing a nonempty TypedArray fails after making it nonextensible
(function(){
            var value=new Uint8Array([7,8]);
            try{Object.freeze(value)}catch(error){
                var descriptor=Object.getOwnPropertyDescriptor(value,"0");
                return error.name+":"+Object.isExtensible(value)+":"+descriptor.value+":"+
                    descriptor.writable+":"+descriptor.configurable;
            }
            return "missing";
        })()
// case: Proxy integrity predicate trap order can return before isExtensible
(function(){
            var log="",base=Object();base.x=1;
            var proxy=new Proxy(base,{
                ownKeys:function(target){log+="keys|";return Reflect.ownKeys(target)},
                getOwnPropertyDescriptor:function(target,key){log+="descriptor-"+key+"|";return Reflect.getOwnPropertyDescriptor(target,key)},
                isExtensible:function(target){log+="extensible|";return Reflect.isExtensible(target)}
            });
            return Object.isFrozen(proxy)+"|"+log;
        })()
