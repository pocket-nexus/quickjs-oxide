// mode: completion-array
// CONSTRUCTOR_CASES
// case: Object call and new allocate ordinary objects
(function(){
            var a=Object(),b=new Object(),c=Object(null),d=Object(undefined);
            return [
                Object.getPrototypeOf(a)===Object.prototype,
                Object.getPrototypeOf(b)===Object.prototype,
                Object.getPrototypeOf(c)===Object.prototype,
                Object.getPrototypeOf(d)===Object.prototype,
                a!==b,b!==c,c!==d
            ];
        })()
// case: Object preserves objects and boxes every primitive family
(function(){
            var original=Object.create(null),symbol=Symbol("object"),big=7n;
            var number=Object(3),boolean=Object(false),string=Object("A"),
                boxedSymbol=Object(symbol),boxedBig=Object(big);
            return [
                Object(original)===original,new Object(original)===original,
                Object.prototype.toString.call(number),number.valueOf(),
                Object.prototype.toString.call(boolean),boolean.valueOf(),
                Object.prototype.toString.call(string),string.valueOf(),string.length,
                Object.prototype.toString.call(boxedSymbol),boxedSymbol.valueOf()===symbol,
                Object.prototype.toString.call(boxedBig),boxedBig.valueOf()===big
            ];
        })()
// case: Object call ignores this and repeated primitive boxing is fresh
(function(){
            var receiver=Object.create(null),a=Object.call(receiver),
                b=Object.call(receiver,1),c=Object.call(receiver,1);
            return [
                Object.getPrototypeOf(a)===Object.prototype,
                Object.prototype.toString.call(b),b!==c,b.valueOf(),c.valueOf()
            ];
        })()
// CREATE_AND_PROTOTYPE_CASES
// case: Object.create installs prototype and descriptor properties
(function(){
            var proto=Object(),descriptors=Object.create(null),x=Object(),y=Object();
            proto.inherited=9;
            x.value=7;x.writable=false;x.enumerable=true;x.configurable=false;
            y.value=8;y.writable=true;y.enumerable=false;y.configurable=true;
            descriptors.x=x;descriptors.y=y;
            var object=Object.create(proto,descriptors),nullObject=Object.create(null);
            var write=(function(){"use strict";try{object.x=10;return "missing"}catch(e){return e.name}})();
            return [
                Object.getPrototypeOf(object)===proto,object.inherited,object.x,object.y,
                object.propertyIsEnumerable("x"),object.propertyIsEnumerable("y"),
                write,delete object.x,delete object.y,
                Object.getPrototypeOf(nullObject)===null
            ];
        })()
// case: Object.create rejects the prototype before reading properties
(function(){
            var log="",properties=Object();
            properties.__defineGetter__("x",function(){log+="x";throw 8});
            try{Object.create(1,properties)}catch(error){return log+"|"+error.name+"|"+error.message}
            return "missing";
        })()
// case: Object getPrototypeOf boxes primitives and rejects nullish values
(function(){
            var numberPrototype=Object.getPrototypeOf(Object(0));
            var stringPrototype=Object.getPrototypeOf(Object(""));
            var booleanPrototype=Object.getPrototypeOf(Object(false));
            function observe(value){try{return Object.getPrototypeOf(value)===Object.prototype?"object":
                Object.getPrototypeOf(value)===numberPrototype?"number":
                Object.getPrototypeOf(value)===stringPrototype?"string":
                Object.getPrototypeOf(value)===booleanPrototype?"boolean":"other"}
                catch(error){return error.name+":"+error.message}}
            return [observe(Object()),observe(1),observe("x"),observe(false),observe(null),observe(undefined)];
        })()
// case: Object.setPrototypeOf returns the target and handles primitives
(function(){
            function observe(thunk){try{return thunk()}catch(error){return error.name+":"+error.message}}
            var first=Object(),second=Object(),fixed=Object.create(null);
            var returned=Object.setPrototypeOf(first,second);
            var same=Object.setPrototypeOf(fixed,null);
            var thrower=Function.prototype.__lookupGetter__("caller");
            return [
                returned===first,Object.getPrototypeOf(first)===second,same===fixed,
                Object.setPrototypeOf(1,second),Object.setPrototypeOf("x",second),
                observe(function(){return Object.setPrototypeOf(null,second)}),
                observe(function(){return Object.setPrototypeOf(Object(),1)}),
                observe(function(){return Object.setPrototypeOf(second,first)}),
                observe(function(){return Object.setPrototypeOf(Object.prototype,Object())}),
                observe(function(){return Object.setPrototypeOf(thrower,Object())})
            ];
        })()
// DEFINE_PROPERTY_CASES
// case: Object.defineProperty applies data and accessor descriptors
(function(){
            var object=Object(),stored=0,data=Object(),accessor=Object();
            data.value=7;data.writable=false;data.enumerable=true;data.configurable=false;
            accessor.get=function(){return stored+1};
            accessor.set=function(value){stored=value};
            accessor.enumerable=false;accessor.configurable=true;
            var dataResult=Object.defineProperty(object,"fixed",data);
            var accessorResult=Object.defineProperty(object,"value",accessor);
            object.value=9;
            var strictWrite=(function(){"use strict";try{object.fixed=8;return "missing"}catch(e){return e.name}})();
            return [
                dataResult===object,accessorResult===object,object.fixed,strictWrite,
                object.propertyIsEnumerable("fixed"),delete object.fixed,
                object.value,stored,object.propertyIsEnumerable("value"),delete object.value
            ];
        })()
// case: property key precedes complete descriptor field conversion
(function(){
            var log="",target=Object(),key=Object(),descriptor=Object();
            key.toString=function(){log+="k";return "x"};
            descriptor.__defineGetter__("enumerable",function(){log+="e";return true});
            descriptor.__defineGetter__("configurable",function(){log+="c";return true});
            descriptor.__defineGetter__("value",function(){log+="v";return 1});
            descriptor.__defineGetter__("writable",function(){log+="w";return true});
            descriptor.__defineGetter__("get",function(){log+="g";return function(){}});
            descriptor.__defineGetter__("set",function(){log+="s";return function(){}});
            try{Object.defineProperty(target,key,descriptor)}
            catch(error){return log+"|"+error.name+"|"+error.message+"|"+target.hasOwnProperty("x")}
            return "missing";
        })()
// case: descriptor field throws preserve the pinned accessor overwrite quirk
(function(){
            function row(field){
                var descriptor=Object();
                descriptor.__defineGetter__(field,function(){throw "sentinel"});
                try{Object.defineProperty(Object(),"x",descriptor)}
                catch(error){
                    return field+"|"+(typeof error==="object"?error.name+":"+error.message:error);
                }
                return "missing";
            }
            return [row("enumerable"),row("get"),row("set")];
        })()
// case: defineProperty validates target then key then descriptor
(function(){
            function row(mode){
                var log="",key=Object(),descriptor=Object();
                key.toString=function(){log+="k";if(mode===1)throw "key";return "x"};
                descriptor.__defineGetter__("value",function(){log+="v";throw "descriptor"});
                try{
                    if(mode===0)Object.defineProperty(null,key,descriptor);
                    else if(mode===2)Object.defineProperty(Object(),key,1);
                    else Object.defineProperty(Object(),key,descriptor);
                }catch(error){return log+":"+(typeof error==="object"?error.name:error)}
                return "missing";
            }
            return [row(0),row(1),row(2),row(3)];
        })()
// case: QuickJS defineProperties converts and defines sequentially
(function(){
            var log="",target=Object(),descriptors=Object(),a=Object();
            a.value=1;a.writable=true;a.enumerable=true;a.configurable=true;
            descriptors.a=a;
            descriptors.__defineGetter__("b",function(){log+="b";throw "stop"});
            try{Object.defineProperties(target,descriptors)}catch(error){
                return [log,error,target.hasOwnProperty("a"),target.a,target.hasOwnProperty("b")];
            }
            return "missing";
        })()
// case: QuickJS snapshots Proxy enumerability before reading descriptor values
(function(){
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
})()
// case: Object.create shares defineProperties partial failure behavior
(function(){
            var log="",descriptors=Object(),a=Object();
            a.value=1;a.writable=true;a.enumerable=true;a.configurable=true;
            descriptors.a=a;
            descriptors.__defineGetter__("b",function(){log+="b";throw 23});
            try{Object.create(null,descriptors)}catch(error){return log+"|"+error}
            return "missing";
        })()
// OWN_KEY_ARRAY_CASES
// case: own property name and symbol arrays partition ordinary keys
(function(){
            function list(array){var text="";for(var i=0;i<array.length;i++){if(i)text+=",";text+=array[i]}return text}
            var first=Symbol("first"),second=Symbol("second"),object=Object(),hidden=Object();
            object.beta=1;object["4294967295"]=2;object["2"]=3;object["0"]=4;
            hidden.value=5;Object.defineProperty(object,"hidden",hidden);
            object[first]=6;object.alpha=7;object[second]=8;
            var names=Object.getOwnPropertyNames(object),symbols=Object.getOwnPropertySymbols(object);
            return [
                Array.isArray(names),Object.getPrototypeOf(names)===Array.prototype,list(names),
                Array.isArray(symbols),Object.getPrototypeOf(symbols)===Array.prototype,
                symbols.length,symbols[0]===first,symbols[1]===second,
                list(Object.getOwnPropertyNames(names)),list(Object.getOwnPropertyNames(symbols))
            ];
        })()
// case: Array exotic own keys become dense key arrays
(function(){
            function list(array){var text="";for(var i=0;i<array.length;i++){if(i)text+=",";text+=array[i]}return text}
            var symbol=Symbol("array"),array=[10,,30];array.foo=4;array[symbol]=5;
            var names=Object.getOwnPropertyNames(array),symbols=Object.getOwnPropertySymbols(array);
            return [
                list(names),names.length,0 in names,1 in names,2 in names,3 in names,
                symbols.length,symbols[0]===symbol,
                list(Object.getOwnPropertyNames(names)),list(Object.getOwnPropertyNames(symbols))
            ];
        })()
// case: String exotic virtual keys merge with stored strings and symbols
(function(){
            function list(array){var text="";for(var i=0;i<array.length;i++){if(i)text+=",";text+=array[i]}return text}
            var symbol=Symbol("string"),wrapper=Object("A\uD83D\uDCA9\uD800");
            wrapper[8]=8;wrapper.foo=9;wrapper[symbol]=10;
            var names=Object.getOwnPropertyNames(wrapper),symbols=Object.getOwnPropertySymbols(wrapper);
            var primitiveNames=Object.getOwnPropertyNames("A\uD83D\uDCA9");
            return [
                list(names),list(primitiveNames),symbols.length,symbols[0]===symbol,
                Array.isArray(names),Object.getPrototypeOf(names)===Array.prototype,
                list(Object.getOwnPropertyNames(names))
            ];
        })()
// case: own key statics box primitives and reject nullish values
(function(){
            function observe(thunk){try{var value=thunk();return value.length}catch(error){return error.name+":"+error.message}}
            return [
                observe(function(){return Object.getOwnPropertyNames(1)}),
                observe(function(){return Object.getOwnPropertySymbols(1)}),
                observe(function(){return Object.getOwnPropertyNames(null)}),
                observe(function(){return Object.getOwnPropertySymbols(undefined)})
            ];
        })()
// PROTOTYPE_SUFFIX_CASES
// case: core Object prototype methods preserve nullish behavior
(function(){
            function observe(method,value){
                try{return method.call(value)}catch(error){return error.name+":"+error.message}
            }
            return [
                observe(Object.prototype.toString,null),
                observe(Object.prototype.toString,undefined),
                observe(Object.prototype.toLocaleString,null),
                observe(Object.prototype.toLocaleString,undefined),
                observe(Object.prototype.valueOf,null),
                observe(Object.prototype.valueOf,undefined)
            ];
        })()
// case: hasOwnProperty and propertyIsEnumerable distinguish own descriptors
(function(){
            var symbol=Symbol("own"),parent=Object(),hidden=Object();parent.inherited=1;
            var object=Object.create(parent);object.visible=2;hidden.value=3;
            Object.defineProperty(object,"hidden",hidden);object[symbol]=4;
            return [
                object.hasOwnProperty("visible"),object.hasOwnProperty("hidden"),
                object.hasOwnProperty("inherited"),object.hasOwnProperty(symbol),
                object.propertyIsEnumerable("visible"),object.propertyIsEnumerable("hidden"),
                object.propertyIsEnumerable("inherited"),object.propertyIsEnumerable(symbol)
            ];
        })()
// case: property probes convert keys before nullish receivers
(function(){
            function run(method){
                var log="",key=Object();key.toString=function(){log+="k";throw 17};
                try{method.call(null,key)}catch(error){return log+":"+error}
                return "missing";
            }
            return [run(Object.prototype.hasOwnProperty),run(Object.prototype.propertyIsEnumerable)];
        })()
// case: isPrototypeOf walks chains and short circuits primitive candidates
(function(){
            function observe(thunk){try{return thunk()}catch(error){return error.name+":"+error.message}}
            var root=Object(),middle=Object.create(root),leaf=Object.create(middle);
            return [
                root.isPrototypeOf(leaf),middle.isPrototypeOf(leaf),leaf.isPrototypeOf(root),
                Object.prototype.isPrototypeOf.call(null,1),
                observe(function(){return Object.prototype.isPrototypeOf.call(null,leaf)})
            ];
        })()
// case: legacy __proto__ getter and setter preserve pinned validation
(function(){
            function observe(thunk){try{return thunk()}catch(error){return error.name+":"+error.message}}
            var parent=Object(),object=Object(),getter=Object.prototype.__lookupGetter__("__proto__"),
                setter=Object.prototype.__lookupSetter__("__proto__");
            var first=setter.call(object,parent),after=object.__proto__===parent;
            var ignored=setter.call(object,1),still=object.__proto__===parent;
            return [
                first,after,ignored,still,getter.call(object)===parent,
                getter.call(1)===Number.prototype,setter.call(1,parent),
                observe(function(){return getter.call(null)}),
                observe(function(){return setter.call(null,parent)}),
                observe(function(){return setter.call(object,object)})
            ];
        })()
// case: legacy accessor definition and lookup walk the prototype chain
(function(){
            var stored=0,parent=Object(),getter=function(){return 41},setter=function(value){stored=value};
            parent.__defineGetter__("answer",getter);parent.__defineSetter__("answer",setter);
            var child=Object.create(parent),before=child.answer;child.answer=9;
            var inheritedGet=child.__lookupGetter__("answer"),inheritedSet=child.__lookupSetter__("answer");
            var data=Object();data.value=3;data.writable=true;data.configurable=true;
            Object.defineProperty(child,"answer",data);
            return [
                before,stored,inheritedGet===getter,inheritedSet===setter,
                parent.propertyIsEnumerable("answer"),
                child.__lookupGetter__("answer"),child.__lookupSetter__("answer"),child.answer
            ];
        })()
// case: legacy accessor helpers preserve receiver callable and key order
(function(){
            function row(mode){
                var log="",receiver=Object(),key=Object(),fn=function(){};
                key.toString=function(){log+="k";throw "key"};
                try{
                    if(mode===0)Object.prototype.__defineGetter__.call(null,key,fn);
                    if(mode===1)receiver.__defineGetter__(key,1);
                    if(mode===2)receiver.__defineGetter__(key,fn);
                    if(mode===3)Object.prototype.__lookupGetter__.call(null,key);
                    if(mode===4)receiver.__lookupGetter__(key);
                }catch(error){return log+":"+(typeof error==="object"?error.name:error)}
                return "missing";
            }
            return [row(0),row(1),row(2),row(3),row(4)];
        })()
