function __units(value) {
    value = String(value);
    var output = "";
    for (var index = 0; index < value.length; index++) {
        if (index) output += ",";
        var unit = value.charCodeAt(index).toString(16);
        while (unit.length < 4) unit = "0" + unit;
        output += unit;
    }
    return output;
}
function __template(raw) {
    var cooked = Object();
    cooked.raw = raw;
    return cooked;
}
function __rawObject(length, zero, one, two) {
    var raw = Object();
    raw.length = length;
    raw[0] = zero;
    raw[1] = one;
    raw[2] = two;
    return raw;
}
function __getter(getter) {
    var descriptor = Object();
    descriptor.configurable = true;
    descriptor.get = getter;
    return descriptor;
}
function __bits(object, key) {
    var descriptor = Object.getOwnPropertyDescriptor(object, key);
    return (descriptor.writable ? "1" : "0") +
           (descriptor.enumerable ? "1" : "0") +
           (descriptor.configurable ? "1" : "0");
}
function __constructor(value) {
    try { new value(); return true; }
    catch (_) { return false; }
}
// GRAPH_CASES
// case: constructor own-key table is complete and ordered
(function(){return Object.getOwnPropertyNames(String).join(",")})()
// case: global String occupies its pinned relative bootstrap position
(function(){
            var selected=["Number","Boolean","String","Math","Reflect","Symbol","globalThis","BigInt"];
            var keys=Object.getOwnPropertyNames(globalThis),output=[];
            for(var index=0;index<keys.length;index++)
                if(selected.indexOf(keys[index])>=0)output.push(keys[index]);
            return output.join(",");
        })()
// case: constructor and prototype descriptors match the function-list graph
(function(){return [
            __bits(globalThis,"String"),__bits(String,"length"),__bits(String,"name"),
            __bits(String,"fromCharCode"),__bits(String,"fromCodePoint"),
            __bits(String,"raw"),__bits(String,"prototype"),
            __bits(String.prototype,"constructor")
        ].join("|")})()
// case: constructor and branded empty prototype are linked
(function(){return [
            typeof String,Object.getPrototypeOf(String)===Function.prototype,
            String.prototype.constructor===String,
            Object.getPrototypeOf(String.prototype)===Object.prototype,
            Object.prototype.toString.call(String.prototype),
            String.prototype.valueOf(),Object.isExtensible(String.prototype),
            __constructor(String)
        ].join("|")})()
// case: static metadata and non-constructor bits are exact
(function(){
            var names=["fromCharCode","fromCodePoint","raw"],output=[];
            for(var index=0;index<names.length;index++){
                var name=names[index],fn=String[name];
                output.push(fn.name+":"+fn.length+":"+
                    Object.getOwnPropertyNames(fn).join(",")+":"+
                    (Object.getPrototypeOf(fn)===Function.prototype)+":"+
                    __constructor(fn));
            }
            return output.join("|");
        })()
// case: implemented prototype keys keep pinned filtered order through constructor
(function(){
            var selected=["length","at","charCodeAt","charAt","concat","codePointAt",
                "isWellFormed","toWellFormed","indexOf","lastIndexOf","includes",
                "endsWith","startsWith","toString","valueOf","constructor"];
            var keys=Object.getOwnPropertyNames(String.prototype),output=[];
            for(var index=0;index<keys.length;index++)
                if(selected.indexOf(keys[index])>=0)output.push(keys[index]);
            return output.join(",");
        })()
// case: lazy statics materialize once and preserve identity
(function(){
            var first=String.fromCodePoint,second=String.fromCodePoint;
            var descriptor=Object.getOwnPropertyDescriptor(String,"fromCodePoint");
            return (first===second)+"|"+(first===descriptor.value)+"|"+
                Object.getOwnPropertyNames(first).join(",");
        })()
// AUTOINIT_CASES
// case: a lazy static can be deleted before first materialization
(function(){
            var deleted=delete String.fromCharCode;
            return [deleted,"fromCharCode" in String,
                Object.prototype.hasOwnProperty.call(String,"fromCharCode"),
                typeof String.fromCharCode,Object.getOwnPropertyNames(String).join(",")].join("|");
        })()
// case: assignment replaces a lazy static with an ordinary value
(function(){
            String.raw=17;
            return [String.raw,__bits(String,"raw"),Object.getOwnPropertyNames(String).join(",")].join("|");
        })()
// case: the eager global constructor itself is configurable
(function(){
            var saved=String;
            var deleted=delete globalThis.String;
            var result=[deleted,"String" in globalThis,
                Object.prototype.hasOwnProperty.call(globalThis,"String"),typeof globalThis.String].join("|");
            globalThis.String=saved;
            return result;
        })()
// CALL_CASES
// case: ordinary calls cover absent and primitive arguments
(function(){
            var values=[String(),String(undefined),String(null),String(false),String(true),
                String(0),String(-0),String(1.5),String(NaN),String(Infinity),
                String(-Infinity),String(1n),String("A\ud800Z")],output=[];
            for(var index=0;index<values.length;index++)output.push(__units(values[index]));
            return output.join("|");
        })()
// case: direct Symbol arguments use the constructor-only formatting exception
(function(){
            var values=[String(Symbol()),String(Symbol("")),String(Symbol("x")),
                String(Symbol("\ud800")),String(Symbol.iterator)],output=[];
            for(var index=0;index<values.length;index++)output.push(__units(values[index]));
            return output.join("|");
        })()
// case: call conversion uses string hint and ordinary fallback order
(function(){
            var log="",exotic=Object(),fallback=Object();
            exotic[Symbol.toPrimitive]=function(hint){log+="exotic:"+hint+";";return "E"};
            fallback.toString=function(){log+="toString;";return Object()};
            fallback.valueOf=function(){log+="valueOf;";return "V"};
            var first=String(exotic),second=String(fallback);
            return __units(first)+"|"+__units(second)+"|"+log;
        })()
// case: call ignores this and all arguments after the first
(function(){
            var hit=false,bomb=Object();
            bomb[Symbol.toPrimitive]=function(){hit=true;throw "late"};
            return __units(String.call(bomb,"first",bomb))+"|"+hit;
        })()
// CONSTRUCT_CASES
// case: new String creates a branded empty wrapper
(function(){
            var value=new String();
            return [typeof value,Object.getPrototypeOf(value)===String.prototype,
                Object.prototype.toString.call(value),__units(value.valueOf()),
                Object.getOwnPropertyNames(value).join(",")].join("|");
        })()
// case: constructed UTF-16 payload exposes virtual indices and fixed length
(function(){
            var value=new String("A\ud800Z"),descriptor=Object.getOwnPropertyDescriptor(value,"length");
            return [__units(value.valueOf()),Object.getOwnPropertyNames(value).join(","),
                descriptor.value,__bits(value,"length"),__bits(value,"0"),
                __units(value[0]),__units(value[1]),__units(value[2])].join("|");
        })()
// case: construction converts its argument exactly once with string hint
(function(){
            var log="",argument=Object();
            argument[Symbol.toPrimitive]=function(hint){log+=hint+";";return "xy"};
            var value=new String(argument);
            return __units(value.valueOf())+"|"+log;
        })()
// FROM_CHAR_CODE_CASES
// case: fromCharCode applies ToInt32 and retains exact low UTF-16 units
(function(){return __units(String.fromCharCode())+"|"+
            __units(String.fromCharCode(65,0xd800,0x1f600,-1,65536,65.9,NaN,Infinity,-Infinity))})()
// case: fromCharCode converts arguments left to right and stops on abrupt completion
(function(){
            var log="";
            function item(name,value,throws){
                var object=Object();
                object[Symbol.toPrimitive]=function(hint){
                    log+=name+":"+hint+";";if(throws)throw name;return value;
                };
                return object;
            }
            var result;
            try{result=String.fromCharCode(item("a",65,false),item("b",66,false),
                item("c",67,true),item("d",68,false))}
            catch(error){result="throw:"+error}
            return result+"|"+log;
        })()
// case: fromCharCode ignores receiver and converts every actual argument
(function(){return __units(String.fromCharCode.call(null,65,66,67))})()
// FROM_CODE_POINT_CASES
// case: fromCodePoint emits BMP surrogate and astral inputs exactly
(function(){return __units(String.fromCodePoint())+"|"+
            __units(String.fromCodePoint(65,0xd800,0x1f600,0x10ffff))})()
// case: fromCodePoint converts arguments left to right
(function(){
            var log="";
            function item(name,value){
                var object=Object();
                object[Symbol.toPrimitive]=function(hint){log+=name+":"+hint+";";return value};
                return object;
            }
            var result=String.fromCodePoint(item("a",65),item("b",0x1f600));
            return __units(result)+"|"+log;
        })()
// case: fromCodePoint ignores receiver
(function(){return __units(String.fromCodePoint.call(17,65,66,67))})()
// RAW_CASES
// case: raw interleaves raw chunks and present substitutions
(function(){return [
            __units(String.raw(__template(["a","b","c"]),1,2)),
            __units(String.raw(__template(["a","b","c"]),1)),
            __units(String.raw(__template(["a","b","c"]))),
            __units(String.raw(__template([]))),__units(String.raw(__template("A\ud800Z"),"-","+"))
        ].join("|")})()
// case: raw observes cooked raw length index and substitution order
(function(){
            var log="",cooked=Object(),raw=Object(),length=Object(),substitution=Object();
            Object.defineProperty(cooked,"raw",__getter(function(){log+="get-raw;";return raw}));
            Object.defineProperty(raw,"length",__getter(function(){log+="get-length;";return length}));
            length[Symbol.toPrimitive]=function(hint){log+="length:"+hint+";";return 2};
            Object.defineProperty(raw,"0",__getter(function(){log+="get-0;";return "A"}));
            Object.defineProperty(raw,"1",__getter(function(){log+="get-1;";return "B"}));
            substitution.toString=function(){log+="sub-toString;";return Object()};
            substitution.valueOf=function(){log+="sub-valueOf;";return "S"};
            var result=String.raw(cooked,substitution);
            return __units(result)+"|"+log;
        })()
// case: raw converts each chunk before its following substitution
(function(){
            var log="",raw=Object(),first=Object(),second=Object(),sub=Object();raw.length=2;
            first.toString=function(){log+="first;";return "A"};
            second.toString=function(){log+="second;";return "B"};
            sub.toString=function(){log+="sub;";return "S"};
            raw[0]=first;raw[1]=second;
            var result=String.raw(__template(raw),sub);
            return __units(result)+"|"+log;
        })()
// case: raw ToLength clamps negative and NaN lengths to zero
(function(){return [
            __units(String.raw(__template(__rawObject(-1,"wrong")))),
            __units(String.raw(__template(__rawObject(NaN,"wrong")))),
            __units(String.raw(__template(__rawObject(2.9,"a","b","wrong"))))
        ].join("|")})()
// case: raw ignores this
(function(){return __units(String.raw.call(Symbol("this"),__template(["x","y"]),"-"))})()
// ERROR_CASES
// case: ordinary ToString rejects a Symbol wrapper
String(Object(Symbol("wrapped")))
// case: new String does not use the direct Symbol exception
new String(Symbol("x"))
// case: String preserves an arbitrary conversion throw
(function(){var value=Object();value[Symbol.toPrimitive]=function(){throw "sentinel"};return String(value)})()
// case: String rejects an object ToPrimitive result
(function(){var value=Object();value[Symbol.toPrimitive]=function(){return Object()};return String(value)})()
// case: fromCharCode rejects BigInt
String.fromCharCode(1n)
// case: fromCharCode rejects Symbol
String.fromCharCode(Symbol('x'))
// case: fromCodePoint rejects a negative
String.fromCodePoint(-1)
// case: fromCodePoint rejects an out-of-range value
String.fromCodePoint(0x110000)
// case: fromCodePoint rejects NaN
String.fromCodePoint(NaN)
// case: fromCodePoint rejects infinity
String.fromCodePoint(Infinity)
// case: fromCodePoint rejects a fraction
String.fromCodePoint(1.5)
// case: fromCodePoint rejects BigInt
String.fromCodePoint(1n)
// case: raw rejects a missing template
String.raw()
// case: raw rejects a null raw value
String.raw(__template(null))
// case: raw rejects a Symbol chunk
String.raw(__template([Symbol('chunk')]))
// case: raw preserves a primitive raw getter throw
(function(){var value=Object();Object.defineProperty(value,"raw",__getter(function(){throw 91}));return String.raw(value)})()
// case: raw stops after an abrupt chunk conversion
(function(){
            var log="",raw=Object(),first=Object(),second=Object();raw.length=2;
            first.toString=function(){log+="first;";throw "chunk"};
            second.toString=function(){log+="second;";return "late"};raw[0]=first;raw[1]=second;
            try{String.raw(__template(raw))}catch(error){return log+error}
            return "missing";
        })()
// case: fromCharCode is not a constructor
new String.fromCharCode()
// case: fromCodePoint is not a constructor
new String.fromCodePoint()
// case: raw is not a constructor
new String.raw()
// EXOTIC_CASES
// case: Proxy raw access exposes every ordered internal Get
(function(){
            var log="",rawTarget={length:2,0:"a",1:"b"};
            var raw=new Proxy(rawTarget,{get:function(target,key,receiver){
                log+="raw:"+String(key)+";";return Reflect.get(target,key,receiver)}});
            var cooked=new Proxy({raw:raw},{get:function(target,key,receiver){
                log+="cooked:"+String(key)+";";return Reflect.get(target,key,receiver)}});
            return String.raw(cooked,"-")+"|"+log;
        })()
// case: Proxy newTarget prototype is read only after argument conversion
(function(){
            var log="",argument=Object();
            argument[Symbol.toPrimitive]=function(hint){log+="argument:"+hint+";";return "x"};
            var target=new Proxy(function(){},{get:function(object,key,receiver){
                log+="target:"+String(key)+";";return Reflect.get(object,key,receiver)}});
            var value=Reflect.construct(String,[argument],target);
            return value.valueOf()+"|"+log;
        })()
// case: RegExp and TypedArray values use ordinary conversion and array-like paths
(function(){return String(/a/g)+"|"+String.raw({raw:new Uint16Array([65,66])},"-")})()
// case: String.raw reads a Proxy-wrapped TypedArray in array-like order
(function(){
            var log="",target=new Uint16Array([65,66]);
            var raw=new Proxy(target,{get:function(object,key){
                log+=String(key)+";";return object[key];
            }});
            return String.raw({raw:raw},"-")+"|"+log;
        })()
