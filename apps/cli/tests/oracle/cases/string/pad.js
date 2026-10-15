// prelude: string_capture
// GRAPH_CASES
// case: prototype own-key order places the pad pair after repeat
(function(){
            var selected=["length","includes","endsWith","startsWith",
                "substring","substr","slice","repeat","padEnd","padStart",
                "toString","valueOf","constructor"];
            var keys=Object.getOwnPropertyNames(String.prototype),output=[],index=0;
            while(index<keys.length){
                if(selected.indexOf(keys[index])>=0)output.push(keys[index]);
                index++;
            }
            return output.join(",");
        })()
// case: GenericMagic descriptors names lengths and metadata are exact
(function(){
            var names=["padEnd","padStart"],output=[],index=0;
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
// case: AutoInit yields stable mutually distinct pad identities
(function(){
            var end=String.prototype.padEnd,start=String.prototype.padStart;
            return [(end===String.prototype.padEnd),(start===String.prototype.padStart),
                (end===Object.getOwnPropertyDescriptor(String.prototype,"padEnd").value),
                (start===Object.getOwnPropertyDescriptor(String.prototype,"padStart").value),
                (end===start),(end===String.prototype.repeat),
                (start===String.prototype.repeat)].join("|");
        })()
// AUTOINIT_CASES
// case: lazy padEnd can be deleted before materialization
(function(){
            var deleted=delete String.prototype.padEnd;
            return [deleted,"padEnd" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"padEnd"),
                typeof String.prototype.padEnd].join("|");
        })()
// case: lazy padStart assignment becomes an ordinary replacement
(function(){
            String.prototype.padStart=17;
            return [String.prototype.padStart,__bits(String.prototype,"padStart"),
                Object.prototype.hasOwnProperty.call(String.prototype,"padStart")].join("|");
        })()
// case: materialized padStart remains deletable
(function(){
            var fn=String.prototype.padStart,deleted=delete String.prototype.padStart;
            return [typeof fn,deleted,"padStart" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"padStart")].join("|");
        })()
// VALUE_CASES
// case: default undefined and primitive targets use saturated truncation
(function(){return [
            "abc".padEnd(),"abc".padStart(),
            "abc".padEnd(undefined),"abc".padStart(undefined),
            "abc".padEnd(null),"abc".padStart(false),
            "abc".padEnd(NaN),"abc".padStart(-0),
            "abc".padEnd(3.9),"abc".padStart(3.9),
            "abc".padEnd(4.9),"abc".padStart("5.9"),"done"
        ].join("|")})()
// case: default empty one-unit and multi-unit fillers are exact
(function(){return [
            "abc".padEnd(6),"abc".padStart(6),
            "abc".padEnd(7,undefined),"abc".padStart(7,undefined),
            "abc".padEnd(7,""),"abc".padStart(7,""),
            "abc".padEnd(7,"x"),"abc".padStart(7,"x"),
            "abc".padEnd(8,"xy"),"abc".padStart(8,"xy"),
            "abc".padEnd(8,7n),"abc".padStart(8,true),
            "abc".padEnd(8,null),"done"
        ].join("|")})()
// case: fractions negative values and infinities preserve Int32Sat behavior
(function(){return [
            "x".padEnd(-Infinity,"_"),"x".padStart(-2147483649,"_"),
            "x".padEnd(-1.9,"_"),"x".padStart(-0.9,"_"),
            "x".padEnd(1.9,"_"),"x".padStart(2.9,"_"),
            "x".padEnd(4.9,"ab"),"x".padStart(4.9,"ab"),
            "x".padEnd(Infinity,""),"x".padStart(Number.MAX_VALUE,""),
            "x".padEnd(2147483648,""),"x".padStart(4294967297,"")
        ].join("|")})()
// case: signed 32-bit boundaries determine whether filler conversion occurs
(function(){
            var points=[2147483647,2147483648,4294967297,Infinity,
                -2147483648,-2147483649,-4294967297,-Infinity],output=[],index=0;
            while(index<points.length){
                var hits=0,filler=Object();
                filler[Symbol.toPrimitive]=function(hint){hits++;return ""};
                output.push("x".padEnd(points[index],filler)+":"+hits);
                hits=0;
                output.push("x".padStart(points[index],filler)+":"+hits);
                index++;
            }
            return output.join("|");
        })()
// case: generic primitive and object receivers are converted
(function(){return [
            String.prototype.padEnd.call(123,5,"x"),
            String.prototype.padStart.call(true,6,"_"),
            String.prototype.padEnd.call(7n,4,"0"),
            String.prototype.padStart.call(Object(),17,"."),
            String.prototype.padEnd.call(new String("xy"),5,"z")
        ].join("|")})()
// case: arguments after filler are completely ignored
(function(){
            var extra=Object(),hits=0;
            extra[Symbol.toPrimitive]=function(){hits++;throw "extra"};
            return [String.prototype.padEnd.call("x",4,"_",extra,Symbol("later"),1n),
                String.prototype.padStart.call("x",4,"_",extra),hits].join("|");
        })()
// UTF16_CASES
// case: astral fillers are truncated at raw UTF-16 code-unit boundaries
(function(){
            var astral="\ud83d\ude00",end="X".padEnd(4,astral),start="X".padStart(4,astral);
            var source="A\ud800B\udc00",lone="\udc00Q\ud800";
            var loneEnd=source.padEnd(11,lone),loneStart=source.padStart(11,lone);
            return [end.length,__units(end),start.length,__units(start),
                __units("X".padEnd(2,astral)),__units("X".padStart(2,astral)),
                source.length,loneEnd.length,__units(loneEnd),
                loneStart.length,__units(loneStart)].join("|");
        })()
// case: rope source and filler cross 8192-code-unit and truncation boundaries
(function(){
            function grow(character,power){
                var value=character,index=0;
                while(index<power){value=value+value;index++}
                return value;
            }
            var source=(grow("a",13)+"\udc00")+"Z";
            var filler=grow("q",13)+"\ud83d\ude00";
            var target=source.length+8193;
            var end=source.padEnd(target,filler),start=source.padStart(target,filler);
            var join=source.length;
            return [source.length,filler.length,end.length,start.length,
                __units(end.slice(join-3,join+4)),__units(end.slice(-5)),
                __units(start.slice(8189,8197)),__units(start.slice(8190,8195)),
                __units(start.slice(8191,8196+source.length).slice(-5))].join("|");
        })()
// case: large padding retains exact source and filler boundaries
(function(){
            var source="",index=0;
            while(index<9000){source+=(index%2)?"x":"y";index++}
            source="\ud800"+source+"\udc00";
            var end=source.padEnd(source.length+9,"Q\ud83d\ude00");
            var start=source.padStart(source.length+9,"Q\ud83d\ude00");
            return [end.length,start.length,__units(end.slice(0,3)),
                __units(end.slice(source.length-2,source.length+5)),
                __units(end.slice(-4)),__units(start.slice(0,5)),
                __units(start.slice(7,12)),__units(start.slice(-3))].join("|");
        })()
// ORDER_CASES
// case: receiver target then filler conversion uses string number string hints
(function(){
            function run(name){
                var log="",receiver=Object(),target=Object(),filler=Object(),extra=Object();
                receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "x"};
                target[Symbol.toPrimitive]=function(hint){log+="target:"+hint+";";return 5.9};
                filler[Symbol.toPrimitive]=function(hint){log+="filler:"+hint+";";return "ab"};
                extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";throw "extra"};
                return String.prototype[name].call(receiver,target,filler,extra)+":"+log;
            }
            return [run("padEnd"),run("padStart")].join("|");
        })()
// case: ordinary ToPrimitive fallback follows the two conversion hints
(function(){
            var log="",receiver=Object(),target=Object(),filler=Object();
            receiver.toString=function(){log+="receiver:toString;";return "x"};
            receiver.valueOf=function(){log+="receiver:valueOf;";throw "wrong receiver order"};
            target.valueOf=function(){log+="target:valueOf;";return 4};
            target.toString=function(){log+="target:toString;";throw "wrong target order"};
            filler.toString=function(){log+="filler:toString;";return "_"};
            filler.valueOf=function(){log+="filler:valueOf;";throw "wrong filler order"};
            return String.prototype.padEnd.call(receiver,target,filler)+"|"+log;
        })()
// case: receiver abrupt completion prevents target filler and extra conversion
(function(){
            var log="",receiver=Object(),target=Object(),filler=Object(),extra=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";throw "receiver"};
            target[Symbol.toPrimitive]=function(hint){log+="target:"+hint+";";return 4};
            filler[Symbol.toPrimitive]=function(hint){log+="filler:"+hint+";";return "_"};
            extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";return "_"};
            try{String.prototype.padStart.call(receiver,target,filler,extra)}
            catch(error){return error+"|"+log}
            return "missing";
        })()
// case: target abrupt completion follows receiver and prevents filler conversion
(function(){
            var log="",receiver=Object(),target=Object(),filler=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "x"};
            target[Symbol.toPrimitive]=function(hint){log+="target:"+hint+";";throw "target"};
            filler[Symbol.toPrimitive]=function(hint){log+="filler:"+hint+";";return "_"};
            try{String.prototype.padEnd.call(receiver,target,filler)}
            catch(error){return error+"|"+log}
            return "missing";
        })()
// case: len equal to target returns before filler conversion
(function(){
            function run(name){
                var log="",target=Object(),filler=Object();
                target[Symbol.toPrimitive]=function(hint){log+="target:"+hint+";";return 6.9};
                filler[Symbol.toPrimitive]=function(hint){log+="filler:"+hint+";";throw "filler"};
                return String.prototype[name].call("abcdef",target,filler)+":"+log;
            }
            return [run("padEnd"),run("padStart")].join("|");
        })()
// case: filler abrupt completion happens only after padding is required
(function(){
            function run(name){
                var log="",target=Object(),filler=Object();
                target[Symbol.toPrimitive]=function(hint){log+="target:"+hint+";";return 4};
                filler[Symbol.toPrimitive]=function(hint){log+="filler:"+hint+";";throw "filler"};
                try{String.prototype[name].call("x",target,filler)}
                catch(error){return error+":"+log}
                return "missing";
            }
            return [run("padEnd"),run("padStart")].join("|");
        })()
// case: empty filler returns after conversion and before cap validation
(function(){
            function run(name){
                var log="",target=Object(),filler=Object(),extra=Object();
                target[Symbol.toPrimitive]=function(hint){log+="target:"+hint+";";return 2147483647};
                filler[Symbol.toPrimitive]=function(hint){log+="filler:"+hint+";";return ""};
                extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";throw "extra"};
                return String.prototype[name].call("x",target,filler,extra)+":"+log;
            }
            return [run("padEnd"),run("padStart")].join("|");
        })()
// case: length-cap rejection occurs after nonempty filler conversion
(function(){
            function run(name){
                var log="",target=Object(),filler=Object();
                target[Symbol.toPrimitive]=function(hint){log+="target:"+hint+";";return 2147483647};
                filler[Symbol.toPrimitive]=function(hint){log+="filler:"+hint+";";return "_"};
                try{String.prototype[name].call("x",target,filler)}
                catch(error){return error.name+":"+error.message+"|"+log}
                return "missing";
            }
            return [run("padEnd"),run("padStart")].join("|");
        })()
// ERROR_CASES
// case: padEnd rejects null receiver before target conversion
String.prototype.padEnd.call(null,4,'_')
// case: padStart rejects undefined receiver
String.prototype.padStart.call(undefined,4,'_')
// case: padEnd rejects Symbol receiver
String.prototype.padEnd.call(Symbol('receiver'),4,'_')
// case: padStart rejects Symbol target
'x'.padStart(Symbol('target'),'_')
// case: padEnd rejects BigInt target
'x'.padEnd(4n,'_')
// case: target ToPrimitive object result is rejected
(function(){
            var target=Object();target[Symbol.toPrimitive]=function(){return Object()};
            return "x".padEnd(target,"_");
        })()
// case: padEnd rejects Symbol filler when padding is required
'x'.padEnd(4,Symbol('filler'))
// case: padStart rejects Symbol filler when padding is required
'x'.padStart(4,Symbol('filler'))
// case: filler ToPrimitive object result is rejected
(function(){
            var filler=Object();filler[Symbol.toPrimitive]=function(){return Object()};
            return "x".padStart(4,filler);
        })()
// case: padEnd target above 30-bit cap has exact RangeError
'x'.padEnd(1073741824,'_')
// case: padEnd is not a constructor
new String.prototype.padEnd()
// case: padStart is not a constructor
new String.prototype.padStart()
// STACK_CASES
// case: recursive receiver conversion throws catchably and runtime recovers
(function(){
            var receiver=Object(),errorName="",errorMessage="";
            receiver[Symbol.toPrimitive]=function(){
                return String.prototype.padEnd.call(receiver,2,"_")
            };
            try{String.prototype.padEnd.call(receiver,2,"_")}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"x".padEnd(4,"_"),"x".padStart(4,"_")].join("|");
        })()
// case: recursive target and filler conversions remain catchable
(function(){
            var target=Object(),filler=Object(),targetError="",fillerError="";
            target[Symbol.toPrimitive]=function(){return "x".padStart(target,"_")};
            try{"x".padStart(target,"_")}catch(error){targetError=error.name}
            filler[Symbol.toPrimitive]=function(){return "x".padEnd(4,filler)};
            try{"x".padEnd(4,filler)}catch(error){fillerError=error.name}
            return [targetError,fillerError,"ok".padEnd(4,"!"),"ok".padStart(4,"!")].join("|");
        })()
// case: pad variants repeat includes and subrange share one recursion guard
(function(){
            var value=Object(),depth=0,errorName="",errorMessage="";
            value[Symbol.toPrimitive]=function(){
                depth++;
                if(depth%5===0)return "x".padEnd(value,"_");
                if(depth%5===1)return "x".padStart(value,"_");
                if(depth%5===2)return "x".repeat(value);
                if(depth%5===3)return "abcdef".slice(value,4);
                return "abcdef".includes("a",value);
            };
            try{"x".padEnd(value,"_")}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"x".padEnd(3,"_"),"x".padStart(3,"_"),
                "ok".repeat(2),"abcdef".slice(1,3),"abcdef".includes("bc")].join("|");
        })()
