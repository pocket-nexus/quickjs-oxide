// prelude: string_capture
// GRAPH_CASES
// case: prototype own-key order places repeat after slice
(function(){
            var selected=["length","includes","endsWith","startsWith",
                "substring","substr","slice","repeat","toString","valueOf","constructor"];
            var keys=Object.getOwnPropertyNames(String.prototype),output=[],index=0;
            while(index<keys.length){
                if(selected.indexOf(keys[index])>=0)output.push(keys[index]);
                index++;
            }
            return output.join(",");
        })()
// case: descriptor name length and callable metadata are exact
(function(){
            var fn=String.prototype.repeat;
            return [__bits(String.prototype,"repeat"),fn.name,fn.length,
                Object.getOwnPropertyNames(fn).join(","),__bits(fn,"length"),
                __bits(fn,"name"),Object.getPrototypeOf(fn)===Function.prototype,
                typeof fn,__isConstructor(fn)].join("|");
        })()
// case: AutoInit materializes one stable repeat identity
(function(){
            var first=String.prototype.repeat,again=String.prototype.repeat;
            var descriptor=Object.getOwnPropertyDescriptor(String.prototype,"repeat");
            return [first===again,first===descriptor.value,
                first===String.prototype.slice,first===String.prototype.substring].join("|");
        })()
// AUTOINIT_CASES
// case: lazy repeat can be deleted before materialization
(function(){
            var deleted=delete String.prototype.repeat;
            return [deleted,"repeat" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"repeat"),
                typeof String.prototype.repeat].join("|");
        })()
// case: lazy repeat assignment becomes an ordinary replacement
(function(){
            String.prototype.repeat=17;
            return [String.prototype.repeat,__bits(String.prototype,"repeat"),
                Object.prototype.hasOwnProperty.call(String.prototype,"repeat")].join("|");
        })()
// case: materialized repeat remains deletable
(function(){
            var fn=String.prototype.repeat,deleted=delete String.prototype.repeat;
            return [typeof fn,deleted,"repeat" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"repeat")].join("|");
        })()
// VALUE_CASES
// case: defaults primitives fractions and negative zero use saturated truncation
(function(){return [
            "ab".repeat(),"ab".repeat(undefined),"ab".repeat(null),
            "ab".repeat(false),"ab".repeat(true),"ab".repeat(NaN),
            "ab".repeat(0),"ab".repeat(-0),"ab".repeat(-0.9),
            "ab".repeat(1),"ab".repeat(2.9),"ab".repeat("3.9")
        ].join("|")})()
// case: generic primitive and object receivers are converted
(function(){return [
            String.prototype.repeat.call(123,2),
            String.prototype.repeat.call(true,2),
            String.prototype.repeat.call(7n,3),
            String.prototype.repeat.call(Object(),2),
            String.prototype.repeat.call(new String("xy"),2)
        ].join("|")})()
// case: arguments after count are completely ignored
(function(){
            var extra=Object(),hits=0;
            extra[Symbol.toPrimitive]=function(){hits++;throw "extra"};
            var value=String.prototype.repeat.call("xy",2,extra,Symbol("later"),1n);
            return value+"|"+hits;
        })()
// case: valid empty fast path retains the complete signed count boundary
(function(){return [
            "".repeat(2147483647).length,
            "".repeat(2147483647.9).length,
            "".repeat(0).length,"".repeat(1).length
        ].join("|")})()
// case: Int64 saturation and result length failures remain distinct
(function(){return [
            __capture(function(){return "".repeat(2147483648).length}),
            __capture(function(){return "x".repeat(9223372036854775808).length}),
            __capture(function(){return "x".repeat(-9223372036854775808).length}),
            __capture(function(){return "x".repeat(Number.MAX_VALUE).length}),
            __capture(function(){return "x".repeat(-Number.MAX_VALUE).length}),
            __capture(function(){return "x".repeat(2147483647).length}),
            __capture(function(){return "ab".repeat(536870912).length}),
            __capture(function(){return "\ud83d\ude00".repeat(536870912).length})
        ].join("|")})()
// UTF16_CASES
// case: repeat preserves astral and lone surrogate code units
(function(){
            var source="A\ud83d\ude00\ud800B\udc00Z",result=source.repeat(2);
            return [source.length,result.length,__units(result)].join("|");
        })()
// case: repeat linearizes ropes crossing the 8192-code-unit boundary
(function(){
            function grow(character,power){
                var value=character,index=0;
                while(index<power){value=value+value;index++}
                return value;
            }
            var left=grow("a",13),right=grow("b",13);
            var source=(left+"\ud83d")+("\ude00"+right)+"\ud800Z";
            var result=source.repeat(2),join=source.length;
            return [source.length,result.length,
                __units(result.slice(8189,8197)),
                __units(result.slice(join-3,join+4)),
                __units(result.slice(-5)),result.charCodeAt(join),
                result.charCodeAt(join+8192)].join("|");
        })()
// case: large repeated output retains exact boundaries without decoding
(function(){
            var source="",index=0;
            while(index<9000){source+=(index%2)?"x":"y";index++}
            source="\ud800"+source+"\udc00";
            var result=source.repeat(3),chunk=source.length;
            return [result.length,__units(result.slice(0,3)),
                __units(result.slice(chunk-2,chunk+2)),
                __units(result.slice(chunk*2-2,chunk*2+2)),
                __units(result.slice(-3))].join("|");
        })()
// ORDER_CASES
// case: receiver then count conversion uses string and number hints
(function(){
            var log="",receiver=Object(),count=Object(),extra=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "ab"};
            count[Symbol.toPrimitive]=function(hint){log+="count:"+hint+";";return 2.9};
            extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";throw "extra"};
            return String.prototype.repeat.call(receiver,count,extra)+"|"+log;
        })()
// case: ordinary ToPrimitive fallback orders differ by conversion hint
(function(){
            var log="",receiver=Object(),count=Object();
            receiver.toString=function(){log+="receiver:toString;";return "xy"};
            receiver.valueOf=function(){log+="receiver:valueOf;";throw "wrong receiver order"};
            count.valueOf=function(){log+="count:valueOf;";return 2};
            count.toString=function(){log+="count:toString;";throw "wrong count order"};
            return String.prototype.repeat.call(receiver,count)+"|"+log;
        })()
// case: receiver abrupt completion prevents count and extra conversion
(function(){
            var log="",receiver=Object(),count=Object(),extra=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";throw "receiver"};
            count[Symbol.toPrimitive]=function(hint){log+="count:"+hint+";";return 2};
            extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";return 3};
            try{String.prototype.repeat.call(receiver,count,extra)}
            catch(error){return error+"|"+log}
            return "missing";
        })()
// case: count abrupt completion occurs after receiver and ignores extra
(function(){
            var log="",receiver=Object(),count=Object(),extra=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "ab"};
            count[Symbol.toPrimitive]=function(hint){log+="count:"+hint+";";throw "count"};
            extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";return 3};
            try{String.prototype.repeat.call(receiver,count,extra)}
            catch(error){return error+"|"+log}
            return "missing";
        })()
// case: empty and count-one fast paths still perform count conversion
(function(){
            var log="",zero=Object(),one=Object();
            zero[Symbol.toPrimitive]=function(hint){log+="zero:"+hint+";";return 2147483647};
            one[Symbol.toPrimitive]=function(hint){log+="one:"+hint+";";return 1};
            return ["".repeat(zero).length,"abc".repeat(one),log].join("|");
        })()
// ERROR_CASES
// case: repeat rejects null receiver before count
String.prototype.repeat.call(null,2147483648)
// case: repeat rejects undefined receiver
String.prototype.repeat.call(undefined,1)
// case: repeat rejects Symbol receiver
String.prototype.repeat.call(Symbol('receiver'),1)
// case: repeat rejects Symbol count
'abc'.repeat(Symbol('count'))
// case: repeat rejects BigInt count
'abc'.repeat(1n)
// case: count ToPrimitive object result is rejected
(function(){
            var count=Object();count[Symbol.toPrimitive]=function(){return Object()};
            return "abc".repeat(count);
        })()
// case: negative count has exact RangeError
'abc'.repeat(-1)
// case: negative infinity has exact RangeError
'abc'.repeat(-Infinity)
// case: positive infinity has exact RangeError
'abc'.repeat(Infinity)
// case: count above Int32 max has exact RangeError
''.repeat(2147483648)
// case: result above String cap has distinct RangeError
'ab'.repeat(536870912)
// case: repeat is not a constructor
new String.prototype.repeat()
// STACK_CASES
// case: recursive receiver conversion throws catchably and runtime recovers
(function(){
            var receiver=Object(),errorName="",errorMessage="";
            receiver[Symbol.toPrimitive]=function(){
                return String.prototype.repeat.call(receiver,1)
            };
            try{String.prototype.repeat.call(receiver,1)}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"ab".repeat(2),"abcdef".slice(1,4)].join("|");
        })()
// case: recursive count conversion is catchable and leaves later calls healthy
(function(){
            var count=Object(),errorName="";
            count[Symbol.toPrimitive]=function(){return "x".repeat(count)};
            try{"x".repeat(count)}catch(error){errorName=error.name}
            return [errorName,"xy".repeat(2),"abcdef".includes("cd")].join("|");
        })()
// case: repeat includes and subrange recursion share one guarded family
(function(){
            var value=Object(),depth=0,errorName="",errorMessage="";
            value[Symbol.toPrimitive]=function(){
                depth++;
                if(depth%3===0)return "x".repeat(value);
                if(depth%3===1)return "abcdef".slice(value,4);
                return "abcdef".includes("a",value);
            };
            try{"x".repeat(value)}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"ok".repeat(2),
                "abcdef".slice(1,3),"abcdef".includes("bc")].join("|");
        })()
