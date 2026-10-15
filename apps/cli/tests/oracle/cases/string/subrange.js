// prelude: string_case
// GRAPH_CASES
// case: prototype own-key order reaches the subrange family
(function(){
            var selected=["length","includes","endsWith","startsWith",
                "substring","substr","slice","toString","valueOf","constructor"];
            var keys=Object.getOwnPropertyNames(String.prototype),output=[],index=0;
            while(index<keys.length){
                if(selected.indexOf(keys[index])>=0)output.push(keys[index]);
                index++;
            }
            return output.join(",");
        })()
// case: descriptors names lengths and callable metadata are exact
(function(){
            var names=["substring","substr","slice"],output=[],index=0;
            while(index<names.length){
                var name=names[index],fn=String.prototype[name];
                output.push(name+":"+__bits(String.prototype,name)+":"+fn.name+":"+fn.length+
                    ":"+Object.getOwnPropertyNames(fn).join(",")+":"+
                    __bits(fn,"length")+":"+__bits(fn,"name")+":"+
                    (Object.getPrototypeOf(fn)===Function.prototype)+":"+
                    (typeof fn)+":"+__isConstructor(fn));
                index++;
            }
            return output.join("|");
        })()
// case: AutoInit yields stable and mutually distinct callables
(function(){
            var substring=String.prototype.substring;
            var substr=String.prototype.substr;
            var slice=String.prototype.slice;
            return [(substring===String.prototype.substring),
                (substr===String.prototype.substr),(slice===String.prototype.slice),
                (substring===Object.getOwnPropertyDescriptor(String.prototype,"substring").value),
                (substring===substr),(substring===slice),(substr===slice)].join("|");
        })()
// AUTOINIT_CASES
// case: lazy substring can be deleted before materialization
(function(){
            var deleted=delete String.prototype.substring;
            return [deleted,"substring" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"substring"),
                typeof String.prototype.substring].join("|");
        })()
// case: lazy substr assignment becomes an ordinary replacement
(function(){
            String.prototype.substr=17;
            return [String.prototype.substr,__bits(String.prototype,"substr"),
                Object.prototype.hasOwnProperty.call(String.prototype,"substr")].join("|");
        })()
// case: materialized slice remains deletable
(function(){
            var fn=String.prototype.slice,deleted=delete String.prototype.slice;
            return [typeof fn,deleted,"slice" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"slice")].join("|");
        })()
// VALUE_CASES
// case: substring defaults clamps and swaps its endpoints
(function(){return [
            "012345".substring(),"012345".substring(undefined),
            "012345".substring(2),"012345".substring(2,undefined),
            "012345".substring(2,4),"012345".substring(4,2),
            "012345".substring(-2,3),"012345".substring(2,-3),
            "012345".substring(99,3),"012345".substring(3,99),
            "012345".substring(6,6),"012345".substring(9,9)
        ].join("|")})()
// case: substr defaults negative starts and length clamping
(function(){return [
            "012345".substr(),"012345".substr(undefined),
            "012345".substr(2),"012345".substr(2,undefined),
            "012345".substr(2,3),"012345".substr(-2),
            "012345".substr(-4,2),"012345".substr(-99,2),
            "012345".substr(2,-1),"012345".substr(2,0),
            "012345".substr(2,99),"012345".substr(99,2)
        ].join("|")})()
// case: slice defaults negative indices and does not swap
(function(){return [
            "012345".slice(),"012345".slice(undefined),
            "012345".slice(2),"012345".slice(2,undefined),
            "012345".slice(2,4),"012345".slice(4,2),
            "012345".slice(-2),"012345".slice(-4,-1),
            "012345".slice(-99,3),"012345".slice(2,-99),
            "012345".slice(99,3),"012345".slice(3,99)
        ].join("|")})()
// case: fractions NaN infinities and negative zero use saturated truncation
(function(){return [
            "012345".substring(1.9,4.9),"012345".substring(-1.9,3.9),
            "012345".substring(NaN,3),"012345".substring(Infinity,2),
            "012345".substring(-Infinity,2),"012345".substring(-0,2),
            "012345".substr(1.9,3.9),"012345".substr(-1.9,1.9),
            "012345".substr(NaN,2),"012345".substr(Infinity,2),
            "012345".substr(-Infinity,2),"012345".substr(1,Infinity),
            "012345".substr(1,-Infinity),
            "012345".slice(1.9,4.9),"012345".slice(-1.9,-0),
            "012345".slice(NaN,2),"012345".slice(Infinity,2),
            "012345".slice(-Infinity,2),"012345".slice(1,Infinity),
            "012345".slice(1,-Infinity)
        ].join("|")})()
// case: signed 32-bit saturation boundaries clamp before subranges
(function(){
            var points=[2147483647,2147483648,4294967297,
                -2147483648,-2147483649,-4294967297];
            var names=["substring","substr","slice"],output=[],i=0,j=0;
            while(i<names.length){
                j=0;
                while(j<points.length){
                    output.push(String.prototype[names[i]].call("012345",points[j],3));
                    output.push(String.prototype[names[i]].call("012345",1,points[j]));
                    j++;
                }
                i++;
            }
            return output.join("|");
        })()
// case: null booleans strings and generic primitive receivers are converted
(function(){return [
            "012345".substring(null,true),"012345".substr(false,"2.9"),
            "012345".slice("1.9","4.9"),
            String.prototype.substring.call(12345,1,4),
            String.prototype.substr.call(true,1,2),
            String.prototype.slice.call(777n,1,2),
            String.prototype.substring.call(Object(),1,7)
        ].join("|")})()
// case: arguments after end are completely ignored
(function(){return [
            "abcdef".substring(1,4,Symbol("extra"),1n),
            "abcdef".substr(1,3,Symbol("extra"),1n),
            "abcdef".slice(1,4,Symbol("extra"),1n)
        ].join("|")})()
// UTF16_CASES
// case: subranges preserve lone and split surrogate code units
(function(){
            var source="A\ud83d\ude00\ud800B\udc00Z";
            return [source.length,
                __units(source.substring(1,2)),__units(source.substring(2,4)),
                __units(source.substring(5,3)),
                __units(source.substr(1,1)),__units(source.substr(2,3)),
                __units(source.substr(-3,2)),
                __units(source.slice(1,2)),__units(source.slice(2,5)),
                __units(source.slice(-3,-1))].join("|");
        })()
// case: subranges cross rope leaves and the 8192-code-unit boundary
(function(){
            function grow(character,power){
                var value=character,index=0;
                while(index<power){value=value+value;index++}
                return value;
            }
            var left=grow("a",13),right=grow("b",13);
            var source=(left+"\ud83d")+("\ude00"+right)+"\ud800Z";
            var middle=source.substring(8191,8196);
            var suffix=source.substr(-4,4);
            var spanning=source.slice(8192,8192+4);
            return [source.length,middle.length,__units(middle),
                suffix.length,__units(suffix),spanning.length,__units(spanning),
                source.substring(1,source.length-1).length,
                source.substr(1,source.length-2).length,
                source.slice(1,-1).length].join("|");
        })()
// case: large subranges retain exact first and last raw units
(function(){
            var source="",index=0;
            while(index<9000){source+=(index%2)?"x":"y";index++}
            source="\ud800"+source+"\udc00";
            var a=source.substring(0,9001);
            var b=source.substr(1,9000);
            var c=source.slice(-9001);
            return [a.length,__units(a.substring(0,2)),__units(a.slice(-2)),
                b.length,__units(b.substring(0,2)),__units(b.slice(-2)),
                c.length,__units(c.substring(0,2)),__units(c.slice(-2))].join("|");
        })()
// ORDER_CASES
// case: receiver start then end conversion order uses string number number hints
(function(){
            function run(name){
                var log="",receiver=Object(),start=Object(),end=Object(),extra=Object();
                receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "abcdef"};
                start[Symbol.toPrimitive]=function(hint){log+="start:"+hint+";";return 1.9};
                end[Symbol.toPrimitive]=function(hint){log+="end:"+hint+";";return 4.9};
                extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";throw "extra"};
                return String.prototype[name].call(receiver,start,end,extra)+":"+log;
            }
            return [run("substring"),run("substr"),run("slice")].join("|");
        })()
// case: receiver abrupt completion prevents every argument conversion
(function(){
            function run(name){
                var log="",receiver=Object(),start=Object(),end=Object();
                receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";throw "receiver"};
                start[Symbol.toPrimitive]=function(hint){log+="start:"+hint+";";return 1};
                end[Symbol.toPrimitive]=function(hint){log+="end:"+hint+";";return 2};
                try{String.prototype[name].call(receiver,start,end)}
                catch(error){return error+":"+log}
                return "missing";
            }
            return [run("substring"),run("substr"),run("slice")].join("|");
        })()
// case: start abrupt completion prevents end and extra conversion
(function(){
            function run(name){
                var log="",start=Object(),end=Object(),extra=Object();
                start[Symbol.toPrimitive]=function(hint){log+="start:"+hint+";";throw "start"};
                end[Symbol.toPrimitive]=function(hint){log+="end:"+hint+";";return 2};
                extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";return 3};
                try{String.prototype[name].call("abcdef",start,end,extra)}
                catch(error){return error+":"+log}
                return "missing";
            }
            return [run("substring"),run("substr"),run("slice")].join("|");
        })()
// case: end abrupt completion occurs after start and still ignores extra
(function(){
            function run(name){
                var log="",start=Object(),end=Object(),extra=Object();
                start[Symbol.toPrimitive]=function(hint){log+="start:"+hint+";";return 1};
                end[Symbol.toPrimitive]=function(hint){log+="end:"+hint+";";throw "end"};
                extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";return 3};
                try{String.prototype[name].call("abcdef",start,end,extra)}
                catch(error){return error+":"+log}
                return "missing";
            }
            return [run("substring"),run("substr"),run("slice")].join("|");
        })()
// case: undefined end skips conversion and later arguments remain unobserved
(function(){
            function run(name){
                var log="",start=Object(),extra=Object();
                start[Symbol.toPrimitive]=function(hint){log+="start:"+hint+";";return 2};
                extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";throw "extra"};
                return String.prototype[name].call("abcdef",start,undefined,extra)+":"+log;
            }
            return [run("substring"),run("substr"),run("slice")].join("|");
        })()
// ERROR_CASES
// case: substring rejects null receiver
String.prototype.substring.call(null,0,1)
// case: substr rejects undefined receiver
String.prototype.substr.call(undefined,0,1)
// case: slice rejects Symbol receiver
String.prototype.slice.call(Symbol('receiver'),0,1)
// case: substring rejects Symbol start
'abc'.substring(Symbol('start'),2)
// case: substr rejects BigInt start
'abc'.substr(1n,2)
// case: slice rejects Symbol end
'abc'.slice(0,Symbol('end'))
// case: substring rejects BigInt end
'abc'.substring(0,1n)
// case: start ToPrimitive object result is rejected
(function(){
            var start=Object();start[Symbol.toPrimitive]=function(){return Object()};
            return "abc".substr(start,1);
        })()
// case: end ToPrimitive object result is rejected
(function(){
            var end=Object();end[Symbol.toPrimitive]=function(){return Object()};
            return "abc".slice(0,end);
        })()
// case: substring is not a constructor
new String.prototype.substring()
// case: substr is not a constructor
new String.prototype.substr()
// case: slice is not a constructor
new String.prototype.slice()
// STACK_CASES
// case: recursive start conversion throws catchably and runtime recovers
(function(){
            var start=Object(),errorName="",errorMessage="";
            start[Symbol.toPrimitive]=function(){return "abcdef".substring(start,4)};
            try{"abcdef".substring(start,4)}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"abcdef".substring(1,4),
                "abcdef".substr(-3,2),"abcdef".slice(2,-1)].join("|");
        })()
// case: interleaved subrange recursion cannot bypass the family guard
(function(){
            var position=Object(),depth=0,errorName="";
            position[Symbol.toPrimitive]=function(){
                depth++;
                if(depth%3===0)return "abcdef".substring(position,4);
                if(depth%3===1)return "abcdef".substr(position,2);
                return "abcdef".slice(position,4);
            };
            try{"abcdef".slice(position,4)}catch(error){errorName=error.name}
            return [errorName,"recovered".substring(1,5),
                "recovered".substr(-3,2),"recovered".slice(2,-1)].join("|");
        })()
// case: recursive end conversion is catchable and leaves later calls healthy
(function(){
            var end=Object(),errorName="";
            end[Symbol.toPrimitive]=function(){return "abcdef".substr(0,end)};
            try{"abcdef".substr(0,end)}catch(error){errorName=error.name}
            return [errorName,"abcdef".substring(0,2),"abcdef".slice(-2)].join("|");
        })()
