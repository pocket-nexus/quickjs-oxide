// DESCRIPTOR_CASES
// case: data and accessor field order flags identity and no getter execution
(function(){
            function bits(object,key){
                var descriptor=Object.getOwnPropertyDescriptor(object,key);
                return (descriptor.writable?1:0)+""+(descriptor.enumerable?1:0)+(descriptor.configurable?1:0);
            }
            function fieldBits(descriptor){
                var names=Object.getOwnPropertyNames(descriptor),out=[];
                for(var i=0;i<names.length;i++)out[out.length]=names[i]+":"+bits(descriptor,names[i]);
                return out.join(",");
            }
            var target=Object(),payload=Object(),data=Object(),calls=0;
            data.value=payload;data.writable=false;data.enumerable=true;data.configurable=false;
            Object.defineProperty(target,"data",data);
            var getter=function(){calls++;return 17},setter=function(value){calls+=value},accessor=Object();
            accessor.get=getter;accessor.set=setter;accessor.enumerable=false;accessor.configurable=true;
            Object.defineProperty(target,"accessor",accessor);
            var first=Object.getOwnPropertyDescriptor(target,"data");
            var second=Object.getOwnPropertyDescriptor(target,"data");
            var access=Object.getOwnPropertyDescriptor(target,"accessor");
            return Object.getOwnPropertyNames(first).join(",")+"|"+fieldBits(first)+"|"+
                (first.value===payload)+":"+first.writable+":"+first.enumerable+":"+first.configurable+":"+(first!==second)+"|"+
                Object.getOwnPropertyNames(access).join(",")+"|"+fieldBits(access)+"|"+
                (access.get===getter)+":"+(access.set===setter)+":"+access.enumerable+":"+access.configurable+":"+calls;
        })()
// case: missing and inherited properties return undefined without consulting getters
(function(){
            var calls=0,proto=Object(),target=Object.create(proto);
            proto.inherited=1;
            target.__defineGetter__("value",function(){calls++;return 2});
            var own=Object.getOwnPropertyDescriptor(target,"value");
            return (Object.getOwnPropertyDescriptor(target,"missing")===undefined)+":"+
                (Object.getOwnPropertyDescriptor(target,"inherited")===undefined)+":"+
                (typeof own.get)+":"+(own.get===target.__lookupGetter__("value"))+":"+calls;
        })()
// case: primitive String Array and symbol-key exotic surfaces
(function(){
            function token(descriptor){
                if(descriptor===undefined)return "undefined";
                var value=descriptor.value;
                return (typeof value)+":"+value+":"+descriptor.writable+":"+descriptor.enumerable+":"+descriptor.configurable;
            }
            var symbol=Symbol("own"),target=Object(),array=[];
            target[symbol]=23;array[2]="C";
            var symbolDescriptor=Object.getOwnPropertyDescriptor(target,symbol);
            return token(Object.getOwnPropertyDescriptor(17,"x"))+"|"+
                token(Object.getOwnPropertyDescriptor(false,"x"))+"|"+
                token(Object.getOwnPropertyDescriptor(9n,"x"))+"|"+
                token(Object.getOwnPropertyDescriptor(Symbol("target"),"x"))+"|"+
                token(Object.getOwnPropertyDescriptor("ab","0"))+"|"+
                token(Object.getOwnPropertyDescriptor("ab","length"))+"|"+
                token(Object.getOwnPropertyDescriptor(array,"2"))+"|"+
                token(Object.getOwnPropertyDescriptor(array,"length"))+"|"+
                (symbolDescriptor.value===23)+":"+symbolDescriptor.writable+":"+
                symbolDescriptor.enumerable+":"+symbolDescriptor.configurable;
        })()
// case: undefined key is converted to the undefined property name
(function(){
            var target=Object(),descriptor=Object();
            descriptor.value=41;descriptor.writable=true;descriptor.enumerable=false;descriptor.configurable=true;
            Object.defineProperty(target,"undefined",descriptor);
            var result=Object.getOwnPropertyDescriptor(target);
            return result.value+":"+result.writable+":"+result.enumerable+":"+result.configurable;
        })()
// CONVERSION_CASES
// case: ToObject rejects nullish targets before converting the key
(function(){
            function row(target){
                var log="",key=Object();
                key[Symbol.toPrimitive]=function(hint){log+=hint;throw "key"};
                try{Object.getOwnPropertyDescriptor(target,key)}
                catch(error){return log+"|"+(typeof error==="object"?error.name+":"+error.message:error)}
                return "missing";
            }
            return row(null)+"||"+row(undefined);
        })()
// case: property key @@toPrimitive uses string hint and thrown value is preserved
(function(){
            var log="",target=Object(),key=Object();target.named=7;
            key[Symbol.toPrimitive]=function(hint){log+=hint;return "named"};
            var descriptor=Object.getOwnPropertyDescriptor(target,key);
            var throwing=Object();
            throwing[Symbol.toPrimitive]=function(hint){log+="/"+hint;throw 73};
            try{Object.getOwnPropertyDescriptor(target,throwing)}catch(error){
                return descriptor.value+":"+log+":"+(error===73);
            }
            return "missing";
        })()
// case: bulk nullish targets throw while every other primitive is boxed
(function(){
            function row(value){
                try{return Object.getOwnPropertyNames(Object.getOwnPropertyDescriptors(value)).join(",")}
                catch(error){return error.name+":"+error.message}
            }
            return row(null)+"|"+row(undefined)+"|"+row(19)+"|"+row(false)+"|"+
                row(21n)+"|"+row(Symbol("bulk"))+"|"+row("ab");
        })()
// case: recursive property-key coercion remains valid below the host safety ceiling
(function(){
            var target=Object();target.x=31;
            function recurse(depth){
                var key=Object();
                key[Symbol.toPrimitive]=function(hint){
                    if(hint!=="string")throw "bad hint";
                    if(depth!==0)recurse(depth-1);
                    return "x";
                };
                return Object.getOwnPropertyDescriptor(target,key).value;
            }
            return recurse(4)+":"+Object.getOwnPropertyDescriptor(target,"x").value;
        })()
// BULK_CASES
// case: bulk includes every own string symbol and nonenumerable key in canonical order
(function(){
            function bits(object,key){
                var descriptor=Object.getOwnPropertyDescriptor(object,key);
                return (descriptor.writable?1:0)+""+(descriptor.enumerable?1:0)+(descriptor.configurable?1:0);
            }
            var calls=0,proto=Object(),target=Object.create(proto),hidden=Object();
            var first=Symbol("first"),second=Symbol("second");
            proto.inherited="I";target.z="Z";target[10]="ten";target[first]="S1";target[2]="two";target.a="A";
            hidden.value="H";hidden.writable=false;hidden.enumerable=false;hidden.configurable=true;
            Object.defineProperty(target,"hidden",hidden);
            target.__defineGetter__("access",function(){calls++;return "getter"});
            target[second]="S2";
            var result=Object.getOwnPropertyDescriptors(target),names=Object.getOwnPropertyNames(result),symbols=Object.getOwnPropertySymbols(result);
            var outer=[];for(var i=0;i<names.length;i++)outer[outer.length]=names[i]+":"+bits(result,names[i]);
            var symbolOuter=[];for(var j=0;j<symbols.length;j++)symbolOuter[symbolOuter.length]=(symbols[j]===first?"first":"second")+":"+bits(result,symbols[j]);
            return names.join(",")+"|"+outer.join(",")+"|"+
                (symbols[0]===first)+":"+(symbols[1]===second)+":"+symbolOuter.join(",")+"|"+
                result[2].value+":"+result[10].value+":"+result.hidden.value+":"+result.hidden.enumerable+"|"+
                Object.getOwnPropertyNames(result.access).join(",")+":"+(result.access.get===target.__lookupGetter__("access"))+":"+calls+"|"+
                (result.inherited===undefined)+":"+result[first].value+":"+result[second].value;
        })()
// case: bulk materializes Object AutoInit descriptors and preserves callable identity
(function(){
            var bulk=Object.getOwnPropertyDescriptors(Object);
            var singular=bulk.getOwnPropertyDescriptor;
            var plural=bulk.getOwnPropertyDescriptors;
            return (singular.value===Object.getOwnPropertyDescriptor)+":"+
                (plural.value===Object.getOwnPropertyDescriptors)+":"+
                singular.writable+":"+singular.enumerable+":"+singular.configurable+":"+
                plural.writable+":"+plural.enumerable+":"+plural.configurable+":"+
                singular.value.name+":"+singular.value.length+":"+plural.value.name+":"+plural.value.length;
        })()
// case: bulk returns fresh outer and nested descriptor objects
(function(){
            var target=Object();target.x=1;
            var first=Object.getOwnPropertyDescriptors(target),second=Object.getOwnPropertyDescriptors(target);
            first.x.value=9;first.extra=3;
            return (first!==second)+":"+(first.x!==second.x)+":"+second.x.value+":"+
                (second.extra===undefined)+":"+target.x;
        })()
// EXOTIC_CASES
// case: Proxy singular descriptor preserves key conversion and trap order
(function(){
            var log="",base=Object(),key=Object(),descriptor=Object();
            descriptor.value=7;descriptor.writable=true;descriptor.enumerable=true;descriptor.configurable=true;
            Object.defineProperty(base,"x",descriptor);
            key[Symbol.toPrimitive]=function(hint){log+="key:"+hint+";";return "x"};
            var proxy=new Proxy(base,{getOwnPropertyDescriptor:function(target,name){
                log+="descriptor:"+String(name)+";";
                return Reflect.getOwnPropertyDescriptor(target,name);
            }});
            var result=Object.getOwnPropertyDescriptor(proxy,key);
            return result.value+":"+result.writable+":"+result.enumerable+":"+result.configurable+"|"+log;
        })()
// case: Proxy singular descriptor invariant failures remain observable
(function(){
            var log="",base=Object.preventExtensions(Object());
            var proxy=new Proxy(base,{getOwnPropertyDescriptor:function(){
                log+="descriptor;";
                return {value:1,writable:true,enumerable:true,configurable:true};
            }});
            try{Object.getOwnPropertyDescriptor(proxy,"x")}catch(error){
                return log+error.name+":"+error.message;
            }
            return "missing";
        })()
// case: pinned QuickJS omits the fixed data descriptor same value check
(function(){
    var target=Object(),fixed=Object();
    fixed.value=2;fixed.writable=false;fixed.enumerable=true;fixed.configurable=false;
    Object.defineProperty(target,"x",fixed);
    var proxy=new Proxy(target,{getOwnPropertyDescriptor:function(){
        return {value:1,writable:false,enumerable:true,configurable:false};
    }});
    var result=Object.getOwnPropertyDescriptor(proxy,"x");
    return result.value+":"+result.writable+":"+result.enumerable+":"+result.configurable+"|"+target.x;
})()
// case: TypedArray singular descriptors cover valid out of range and minus zero indices
(function(){
            function token(descriptor){
                if(descriptor===undefined)return "undefined";
                return descriptor.value+":"+descriptor.writable+":"+descriptor.enumerable+":"+descriptor.configurable;
            }
            var value=new Uint8Array([7,8]);
            return token(Object.getOwnPropertyDescriptor(value,0))+"|"+
                token(Object.getOwnPropertyDescriptor(value,"1"))+"|"+
                token(Object.getOwnPropertyDescriptor(value,2))+"|"+
                token(Object.getOwnPropertyDescriptor(value,"-0"))+"|"+
                token(Object.getOwnPropertyDescriptor(value,"length"));
        })()
// case: Proxy bulk descriptors snapshot ownKeys and recheck every descriptor in order
(function(){
            var log="",base={a:1,b:2,gone:3};
            var proxy=new Proxy(base,{
                ownKeys:function(){log+="keys;";return ["b","gone","a"]},
                getOwnPropertyDescriptor:function(target,key){
                    log+="desc:"+key+";";
                    if(key==="gone")return undefined;
                    return Reflect.getOwnPropertyDescriptor(target,key);
                }
            });
            var result=Object.getOwnPropertyDescriptors(proxy);
            return Object.getOwnPropertyNames(result).join(",")+"|"+
                result.b.value+":"+result.a.value+"|"+log;
        })()
// case: TypedArray bulk descriptors materialize integer indexed elements
(function(){
            var result=Object.getOwnPropertyDescriptors(new Uint8Array([7,8]));
            var descriptor=result[0];
            return Object.getOwnPropertyNames(result).join(",")+"|"+
                result[0].value+":"+result[1].value+":"+
                descriptor.writable+":"+descriptor.enumerable+":"+descriptor.configurable;
        })()
