function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+
        __bit(descriptor.configurable);
}
function __isConstructor(value){
    try{Reflect.construct(function(){},[],value);return true}
    catch(_error){return false}
}
function __units(value){
    var output="",index=0,unit;
    while(index<value.length){
        unit=value.charCodeAt(index).toString(16);
        while(unit.length<4)unit="0"+unit;
        if(index!==0)output+=",";
        output+=unit;
        index++;
    }
    return value.length+":"+output;
}
function __arrayUnits(value){
    var output=[],index=0;
    while(index<value.length){output[index]=__units(value[index]);index++}
    return value.length+"["+output.join(";")+"]";
}
function __completion(callback){
    try{return "return:"+String(callback())}
    catch(error){
        if(error!==null&&typeof error==="object")
            return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
// GRAPH_CASES
// case: split occupies the pinned filtered prototype key slot
(function(){
            var selected=["startsWith","split","substring","substr","slice","repeat"],
                keys=Object.getOwnPropertyNames(String.prototype),output=[],index=0;
            while(index<keys.length){
                if(selected.indexOf(keys[index])>=0)output[output.length]=keys[index];
                index++;
            }
            return output.join(",");
        })()
// case: split exposes exact descriptor name length function graph and own keys
(function(){
            var fn=String.prototype.split;
            return [
                __bits(String.prototype,"split"),fn.name,fn.length,
                Object.getOwnPropertyNames(fn).join(","),
                __bits(fn,"name"),__bits(fn,"length"),
                Object.getPrototypeOf(fn)===Function.prototype,
                typeof fn,__isConstructor(fn),
                fn===Object.getOwnPropertyDescriptor(String.prototype,"split").value,
                fn===String.prototype.split
            ].join("|");
        })()
// case: split is distinct from adjacent methods and stable after materialization
(function(){
            var split=String.prototype.split;
            return [split===String.prototype.split,split===String.prototype.startsWith,
                split===String.prototype.substring,split===String.prototype.slice,
                split===String.prototype.repeat].join("|");
        })()
// AUTOINIT_CASES
// case: split can be deleted before materialization
(function(){
            var deleted=delete String.prototype.split;
            return [deleted,"split" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"split"),
                typeof String.prototype.split].join("|");
        })()
// case: split assignment before materialization creates an ordinary replacement
(function(){
            String.prototype.split=17;
            return [String.prototype.split,__bits(String.prototype,"split"),
                Object.prototype.hasOwnProperty.call(String.prototype,"split")].join("|");
        })()
// case: materialized split remains configurable
(function(){
            var fn=String.prototype.split,deleted=delete String.prototype.split;
            return [typeof fn,deleted,"split" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"split")].join("|");
        })()
// DELEGATION_CASES
// case: @@split getter and call receive the raw receiver limit and separator this
(function(){
            var log="",receiver=Object(),limit=Object(),separator=Object(),result=Object();
            receiver.toString=function(){log+="receiver-tostring;";throw "receiver-converted"};
            limit.valueOf=function(){log+="limit-valueof;";throw "limit-converted"};
            separator.toString=function(){log+="separator-tostring;";throw "separator-converted"};
            Object.defineProperty(separator,Symbol.split,{configurable:true,get:function(){
                log+="split-get;";
                return function(source,rawLimit){
                    log+="split-call:"+(this===separator)+":"+(source===receiver)+":"+
                        (rawLimit===limit)+":"+arguments.length+";";
                    return result;
                };
            }});
            var actual=String.prototype.split.call(receiver,separator,limit);
            return log+"result:"+(actual===result);
        })()
// case: @@split observes unboxed primitive receivers
(function(){
            var separator=Object(),output=[];
            separator[Symbol.split]=function(value,limit){
                return [typeof value,String(value),typeof limit,String(limit),
                    this===separator,arguments.length].join(":");
            };
            output[0]=String.prototype.split.call("xy",separator,1);
            output[1]=String.prototype.split.call(7,separator,true);
            output[2]=String.prototype.split.call(false,separator,7n);
            output[3]=String.prototype.split.call(9n,separator,"limit");
            output[4]=String.prototype.split.call(Symbol.iterator,separator,null);
            return output.join("|");
        })()
// case: primitive separators never consult boxed prototype @@split getters
(function(){
            var hits="",output=[];
            function poison(prototype,label){
                Object.defineProperty(prototype,Symbol.split,{configurable:true,get:function(){
                    hits+=label;throw label;
                }});
            }
            poison(String.prototype,"string;");
            poison(Number.prototype,"number;");
            poison(Boolean.prototype,"boolean;");
            poison(BigInt.prototype,"bigint;");
            poison(Symbol.prototype,"symbol;");
            output[0]=__arrayUnits("a,b".split(","));
            output[1]=__arrayUnits("a1b1".split(1));
            output[2]=__arrayUnits("atrueb".split(true));
            output[3]=__arrayUnits("a7b7".split(7n));
            output[4]=__completion(function(){return "abc".split(Symbol.iterator)});
            return output.join("|")+"|hits:"+hits;
        })()
// case: null and undefined @@split values fall back after one getter
(function(){
            function run(splitter){
                var log="",separator=Object();
                Object.defineProperty(separator,Symbol.split,{get:function(){log+="get;";return splitter}});
                separator.toString=function(){log+="string;";return ","};
                return __arrayUnits("a,b".split(separator,3))+":"+log;
            }
            return run(null)+"|"+run(undefined);
        })()
// case: noncallable @@split fails before any ordinary conversion
(function(){
            var log="",receiver=Object(),limit=Object(),separator=Object();
            receiver.toString=function(){log+="receiver;";return "a,b"};
            limit.valueOf=function(){log+="limit;";return 2};
            separator.toString=function(){log+="separator;";return ","};
            Object.defineProperty(separator,Symbol.split,{get:function(){log+="get;";return 17}});
            var completion=__completion(function(){
                return String.prototype.split.call(receiver,separator,limit);
            });
            return completion+"|"+log;
        })()
// case: @@split getter abrupt completion preserves identity and prevents conversion
(function(){
            var log="",sentinel=Object(),receiver=Object(),limit=Object(),separator=Object(),same=false;
            receiver.toString=function(){log+="receiver;";return "x"};
            limit.valueOf=function(){log+="limit;";return 1};
            Object.defineProperty(separator,Symbol.split,{get:function(){log+="get;";throw sentinel}});
            try{String.prototype.split.call(receiver,separator,limit)}
            catch(error){same=error===sentinel}
            return same+"|"+log;
        })()
// case: nullish receiver rejection precedes @@split lookup
(function(){
            var hits=0,separator=Object(),first,second;
            Object.defineProperty(separator,Symbol.split,{get:function(){hits++;return function(){return 1}}});
            first=__completion(function(){return String.prototype.split.call(null,separator,1)});
            second=__completion(function(){return String.prototype.split.call(undefined,separator,1)});
            return first+"|"+second+"|hits:"+hits;
        })()
// VALUE_CASES
// case: undefined null and primitive separators retain pinned special cases
(function(){
            return [
                __arrayUnits("abc".split()),
                __arrayUnits("abc".split(undefined)),
                __arrayUnits("undefined".split(undefined)),
                __arrayUnits("abc".split(null)),
                __arrayUnits("atrueb".split(true)),
                __arrayUnits("a17b17".split(17)),
                __arrayUnits(String.prototype.split.call(12321,2))
            ].join("|");
        })()
// case: empty source and separator cases are exact
(function(){
            return [
                __arrayUnits("".split()),__arrayUnits("".split(undefined)),
                __arrayUnits("".split("")),__arrayUnits("".split("x")),
                __arrayUnits("abc".split("")),__arrayUnits("abc".split("x")),
                __arrayUnits("abc".split("abc")),__arrayUnits("abc".split("abcd")),
                __arrayUnits("aaaa".split("aa")),__arrayUnits("ababa".split("aba"))
            ].join("|");
        })()
// case: ordinary separators copy exact tails and repeated matches
(function(){
            return [
                __arrayUnits("a,b,c".split(",")),
                __arrayUnits(",a,,b,".split(",")),
                __arrayUnits("one<>two<>three".split("<>")),
                __arrayUnits("abababa".split("ba")),
                __arrayUnits("xx--yy--".split("--"))
            ].join("|");
        })()
// case: generic receivers and separator ToString results are exact
(function(){
            var receiver=Object(),separator=Object();
            receiver.toString=function(){return "left::right::tail"};
            separator.valueOf=function(){return Object()};
            separator.toString=function(){return "::"};
            return [__arrayUnits(String.prototype.split.call(receiver,separator)),
                __arrayUnits(String.prototype.split.call(true,"r")),
                __arrayUnits(String.prototype.split.call(9009,0)),
                __arrayUnits(String.prototype.split.call(5n,""))].join("|");
        })()
// UTF16_CASES
// case: empty separator splits UTF-16 code units including astral pairs
(function(){
            var astral="A\ud83d\ude00B",lone="\ud800X\udc00",mixed="\ud83d\ude00\ud800\udc00";
            return [__units(astral),__arrayUnits(astral.split("")),
                __units(lone),__arrayUnits(lone.split("")),
                __units(mixed),__arrayUnits(mixed.split(""))].join("|");
        })()
// case: wide separators match raw code units without scalar normalization
(function(){
            var source="A\ud83d\ude00B\ud83d\ude00C\ud800D\ud800E";
            return [
                __arrayUnits(source.split("\ud83d\ude00")),
                __arrayUnits(source.split("\ud83d")),
                __arrayUnits(source.split("\ude00")),
                __arrayUnits(source.split("\ud800")),
                __arrayUnits(source.split("\ud800D"))
            ].join("|");
        })()
// case: UTF-16 limits can end between surrogate halves
(function(){
            var source="\ud83d\ude00\ud83d\ude00";
            return [__arrayUnits(source.split("",1)),__arrayUnits(source.split("",2)),
                __arrayUnits(source.split("",3)),__arrayUnits(source.split("",4)),
                __arrayUnits(source.split("",5))].join("|");
        })()
// LIMIT_CASES
// case: ToUint32 boundary values control result length
(function(){
            var limits=[undefined,0,-0,NaN,Infinity,-Infinity,-1,-1.9,1.9,
                2,3,4294967295,4294967296,4294967297,8589934593],output=[],index=0;
            while(index<limits.length){
                output[index]=String(limits[index])+":"+__arrayUnits("a,b,c".split(",",limits[index]));
                index++;
            }
            return output.join("|");
        })()
// case: zero limit still converts the separator after receiver and limit
(function(){
            var log="",receiver=Object(),limit=Object(),separator=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "a,b"};
            limit[Symbol.toPrimitive]=function(hint){log+="limit:"+hint+";";return 0};
            Object.defineProperty(separator,Symbol.split,{get:function(){log+="split-get;";return undefined}});
            separator[Symbol.toPrimitive]=function(hint){log+="separator:"+hint+";";return ","};
            return __arrayUnits(String.prototype.split.call(receiver,separator,limit))+"|"+log;
        })()
// case: undefined separator obeys converted limit without matching its text
(function(){
            return [__arrayUnits("--undefined--".split(undefined,0)),
                __arrayUnits("--undefined--".split(undefined,1)),
                __arrayUnits("--undefined--".split(undefined,2)),
                __arrayUnits("--undefined--".split("undefined",1)),
                __arrayUnits("--undefined--".split("undefined",2))].join("|");
        })()
// case: arguments after limit are ignored without conversion
(function(){
            var hits=0,extra=Object();
            extra[Symbol.toPrimitive]=function(){hits++;throw "extra"};
            return __arrayUnits(String.prototype.split.call("a,b,c",",",2,extra,Symbol.iterator,7n))+
                "|hits:"+hits;
        })()
// ORDER_CASES
// case: ordinary path observes @@split then receiver limit separator conversion
(function(){
            var log="",receiver=Object(),limit=Object(),separator=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "a,b,c"};
            limit[Symbol.toPrimitive]=function(hint){log+="limit:"+hint+";";return 2};
            Object.defineProperty(separator,Symbol.split,{get:function(){log+="split-get;";return undefined}});
            separator[Symbol.toPrimitive]=function(hint){log+="separator:"+hint+";";return ","};
            var result=String.prototype.split.call(receiver,separator,limit);
            return __arrayUnits(result)+"|"+log;
        })()
// case: receiver abrupt completion follows @@split lookup and stops later conversion
(function(){
            var log="",sentinel=Object(),receiver=Object(),limit=Object(),separator=Object(),same=false;
            Object.defineProperty(separator,Symbol.split,{get:function(){log+="split-get;";return undefined}});
            receiver[Symbol.toPrimitive]=function(){log+="receiver;";throw sentinel};
            limit.valueOf=function(){log+="limit;";return 1};
            separator.toString=function(){log+="separator;";return ","};
            try{String.prototype.split.call(receiver,separator,limit)}catch(error){same=error===sentinel}
            return same+"|"+log;
        })()
// case: limit abrupt completion follows receiver and stops separator ToString
(function(){
            var log="",sentinel=Object(),receiver=Object(),limit=Object(),separator=Object(),same=false;
            Object.defineProperty(separator,Symbol.split,{get:function(){log+="split-get;";return undefined}});
            receiver.toString=function(){log+="receiver;";return "a,b"};
            limit.valueOf=function(){log+="limit;";throw sentinel};
            separator.toString=function(){log+="separator;";return ","};
            try{String.prototype.split.call(receiver,separator,limit)}catch(error){same=error===sentinel}
            return same+"|"+log;
        })()
// case: separator abrupt completion follows completed limit conversion
(function(){
            var log="",sentinel=Object(),receiver=Object(),limit=Object(),separator=Object(),same=false;
            Object.defineProperty(separator,Symbol.split,{get:function(){log+="split-get;";return undefined}});
            receiver.toString=function(){log+="receiver;";return "a,b"};
            limit.valueOf=function(){log+="limit;";return 1};
            separator.toString=function(){log+="separator;";throw sentinel};
            try{String.prototype.split.call(receiver,separator,limit)}catch(error){same=error===sentinel}
            return same+"|"+log;
        })()
// ERROR_CASES
// case: null receiver is rejected
String.prototype.split.call(null,',',1)
// case: undefined receiver is rejected
String.prototype.split.call(undefined,',',1)
// case: Symbol separator cannot take the ordinary ToString path
'abc'.split(Symbol.iterator)
// case: BigInt limit cannot be converted with ToUint32
'a,b'.split(',',1n)
// case: Symbol limit cannot be converted with ToUint32
'a,b'.split(',',Symbol.iterator)
// case: object-valued limit primitive conversion is rejected
(function(){
            var limit=Object();limit[Symbol.toPrimitive]=function(){return Object()};
            return "a,b".split(",",limit);
        })()
// case: object-valued separator primitive conversion is rejected
(function(){
            var separator=Object();separator[Symbol.toPrimitive]=function(){return Object()};
            return "a,b".split(separator,2);
        })()
// case: noncallable @@split is rejected
(function(){var separator=Object();separator[Symbol.split]=17;return "a".split(separator)})()
// case: split is not a constructor
new String.prototype.split()
// RECURSION_CASES
// case: recursive receiver conversion is catchable and the runtime recovers
(function(){
            var receiver=Object(),name="";
            receiver.toString=function(){return String.prototype.split.call(receiver,",",1)};
            try{String.prototype.split.call(receiver,",",1)}catch(error){name=error.name}
            return name+"|"+__arrayUnits("a,b".split(","));
        })()
// case: recursive limit conversion is catchable and the runtime recovers
(function(){
            var limit=Object(),name="";
            limit.valueOf=function(){return "a,b".split(",",limit)};
            try{"a,b".split(",",limit)}catch(error){name=error.name}
            return name+"|"+__arrayUnits("x-y".split("-",1));
        })()
// case: recursive @@split delegation is catchable and the runtime recovers
(function(){
            var separator=Object(),name="";
            separator[Symbol.split]=function(source,limit){return source.split(separator,limit)};
            try{"a,b".split(separator,2)}catch(error){name=error.name}
            return name+"|"+__arrayUnits("x-y".split("-"));
        })()
