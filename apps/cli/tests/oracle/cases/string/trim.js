// prelude: string_capture
// GRAPH_CASES
// case: prototype own-key order places all five trim properties after padding
(function(){
            var selected=["repeat","padEnd","padStart","trim","trimEnd",
                "trimRight","trimStart","trimLeft","toString","valueOf","constructor"];
            var keys=Object.getOwnPropertyNames(String.prototype),output=[],index=0;
            while(index<keys.length){
                if(selected.indexOf(keys[index])>=0)output.push(keys[index]);
                index++;
            }
            return output.join(",");
        })()
// case: five descriptors and canonical callable metadata are exact
(function(){
            var names=["trim","trimEnd","trimRight","trimStart","trimLeft"];
            var output=[],index=0;
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
// case: canonical methods are stable and aliases initially share their callable
(function(){
            var trim=String.prototype.trim,end=String.prototype.trimEnd;
            var right=String.prototype.trimRight,start=String.prototype.trimStart;
            var left=String.prototype.trimLeft;
            return [trim===String.prototype.trim,
                end===String.prototype.trimEnd,right===String.prototype.trimRight,
                start===String.prototype.trimStart,left===String.prototype.trimLeft,
                end===right,start===left,trim===end,trim===start,end===start,
                trim===Object.getOwnPropertyDescriptor(String.prototype,"trim").value,
                right===Object.getOwnPropertyDescriptor(String.prototype,"trimRight").value,
                left===Object.getOwnPropertyDescriptor(String.prototype,"trimLeft").value,
                right.name,left.name].join("|");
        })()
// PROPERTY_CASES
// case: lazy trim can be deleted before materialization
(function(){
            var deleted=delete String.prototype.trim;
            return [deleted,"trim" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"trim"),
                typeof String.prototype.trim].join("|");
        })()
// case: lazy trim assignment becomes an ordinary replacement
(function(){
            String.prototype.trim=17;
            return [String.prototype.trim,__bits(String.prototype,"trim"),
                Object.prototype.hasOwnProperty.call(String.prototype,"trim")].join("|");
        })()
// case: trimEnd and trimRight properties mutate independently in both directions
(function(){
            var originalEnd=String.prototype.trimEnd;
            var originalRight=String.prototype.trimRight;
            String.prototype.trimEnd=17;
            var rightSurvived=String.prototype.trimRight===originalRight;
            String.prototype.trimRight=23;
            var endStayed=String.prototype.trimEnd===17;
            var deleted=delete String.prototype.trimEnd;
            return [originalEnd===originalRight,rightSurvived,endStayed,deleted,
                Object.prototype.hasOwnProperty.call(String.prototype,"trimEnd"),
                String.prototype.trimRight,__bits(String.prototype,"trimRight")].join("|");
        })()
// case: trimStart and trimLeft properties mutate independently in both directions
(function(){
            var originalStart=String.prototype.trimStart;
            var originalLeft=String.prototype.trimLeft;
            String.prototype.trimLeft=29;
            var startSurvived=String.prototype.trimStart===originalStart;
            String.prototype.trimStart=31;
            var leftStayed=String.prototype.trimLeft===29;
            var deleted=delete String.prototype.trimLeft;
            return [originalStart===originalLeft,startSurvived,leftStayed,deleted,
                Object.prototype.hasOwnProperty.call(String.prototype,"trimLeft"),
                String.prototype.trimStart,__bits(String.prototype,"trimStart")].join("|");
        })()
// WHITESPACE_CASES
// case: all twenty-five pinned whitespace code units trim at either end
(function(){
            var spaces=["\u0009","\u000a","\u000b","\u000c","\u000d","\u0020",
                "\u00a0","\u1680","\u2000","\u2001","\u2002","\u2003","\u2004",
                "\u2005","\u2006","\u2007","\u2008","\u2009","\u200a","\u2028",
                "\u2029","\u202f","\u205f","\u3000","\ufeff"];
            var output=[spaces.length],index=0;
            while(index<spaces.length){
                var unit=spaces[index],sample=unit+"X"+unit;
                output.push(unit.charCodeAt(0).toString(16)+":"+
                    (sample.trim()==="X")+":"+
                    (sample.trimStart()==="X"+unit)+":"+
                    (sample.trimEnd()===unit+"X"));
                index++;
            }
            return output.join("|");
        })()
// case: Unicode and format near-misses are not treated as whitespace
(function(){
            var values=["\u0085","\u180e","\u200b","\u200c","\u200d","\u2060",
                "\u0000","\u001c","\ufff9"];
            var output=[values.length],index=0;
            while(index<values.length){
                var unit=values[index],sample=unit+"X"+unit;
                output.push(unit.charCodeAt(0).toString(16)+":"+
                    __units(sample.trim())+":"+__units(sample.trimStart())+":"+
                    __units(sample.trimEnd()));
                index++;
            }
            return output.join("|");
        })()
// case: trim variants remove only their selected outer runs
(function(){
            var source="\u0020\u0009A\u00a0B\u000a\u3000";
            return [__units(source),__units(source.trim()),
                __units(source.trimStart()),__units(source.trimEnd()),
                __units(source.trimLeft()),__units(source.trimRight())].join("|");
        })()
// case: empty no-space and all-space inputs retain exact values
(function(){
            var all="\u0009\u0020\u00a0\u1680\u2000\u200a\u2028\u2029\u202f\u205f\u3000\ufeff";
            return ["".trim().length,"plain".trim(),"plain".trimStart(),
                "plain".trimEnd(),all.trim().length,all.trimStart().length,
                all.trimEnd().length,all.trimLeft().length,all.trimRight().length].join("|");
        })()
// case: internal whitespace is preserved after both outer scans
(function(){
            var value="\u0009A\u0020\u00a0\u2028B\u000d";
            return [__units(value.trim()),__units(value.trimStart()),
                __units(value.trimEnd())].join("|");
        })()
// UTF16_CASES
// case: lone surrogates and astral pairs remain exact raw UTF-16 units
(function(){
            var value="\u0020\ud800\ud83d\ude00\udc00\u3000";
            var guarded="\u0020\ud800\u0020X\u3000\udc00\u0020";
            return [value.length,__units(value.trim()),__units(value.trimStart()),
                __units(value.trimEnd()),__units(guarded.trim()),
                __units(guarded.trimStart()),__units(guarded.trimEnd())].join("|");
        })()
// case: rope scans cross 8192-code-unit boundaries at both ends
(function(){
            function grow(character,power){
                var value=character,index=0;
                while(index<power){value=value+value;index++}
                return value;
            }
            var left=grow("\u0020",13),right=grow("\u3000",13);
            var middle="\ud800A\ud83d\ude00B\udc00";
            var source=(left+middle)+right;
            var both=source.trim(),start=source.trimStart(),end=source.trimEnd();
            return [left.length,middle.length,right.length,source.length,
                both.length,__units(both),start.length,
                __units(start.slice(0,middle.length+2)),__units(start.slice(-3)),
                end.length,__units(end.slice(0,3)),__units(end.slice(-middle.length))].join("|");
        })()
// case: alternating rope leaves stop on the first interior non-space unit
(function(){
            function grow(character,power){
                var value=character,index=0;
                while(index<power){value=value+value;index++}
                return value;
            }
            var a=grow("\u00a0",12),b=grow("\ufeff",12);
            var source=((a+b)+"X\ud800\u0020Y\udc00")+(b+a);
            var result=source.trim();
            return [source.length,result.length,__units(result),
                __units(source.trimStart().slice(0,7)),
                __units(source.trimEnd().slice(-7))].join("|");
        })()
// ORDER_CASES
// case: generic receivers use a string hint and every extra argument is ignored
(function(){
            function run(name){
                var log="",hits=0,receiver=Object(),extra=Object();
                receiver[Symbol.toPrimitive]=function(hint){
                    log+="receiver:"+hint+";";return "\u0020X\u000a"
                };
                extra[Symbol.toPrimitive]=function(hint){hits++;throw "extra:"+hint};
                var result=String.prototype[name].call(receiver,extra,extra,Symbol("later"),1n);
                return name+":"+__units(result)+":"+log+":"+hits;
            }
            return [run("trim"),run("trimEnd"),run("trimRight"),
                run("trimStart"),run("trimLeft")].join("|");
        })()
// case: ordinary ToPrimitive fallback calls toString before valueOf
(function(){
            var log="",receiver=Object(),extra=Object();
            receiver.toString=function(){log+="toString;";return "\u0020X\u0020"};
            receiver.valueOf=function(){log+="valueOf;";throw "wrong order"};
            extra[Symbol.toPrimitive]=function(){log+="extra;";throw "extra"};
            return String.prototype.trim.call(receiver,extra)+"|"+log;
        })()
// case: receiver abrupt completion preserves identity and prevents extra conversion
(function(){
            function run(name){
                var sentinel=Object(),receiver=Object(),extra=Object(),hits=0;
                receiver[Symbol.toPrimitive]=function(hint){hits++;throw sentinel};
                extra[Symbol.toPrimitive]=function(hint){hits+=100;throw extra};
                try{String.prototype[name].call(receiver,extra)}
                catch(error){return name+":"+(error===sentinel)+":"+hits}
                return "missing";
            }
            return [run("trim"),run("trimEnd"),run("trimStart")].join("|");
        })()
// case: a Symbol.toPrimitive Symbol result fails after the single receiver conversion
(function(){
            var log="",receiver=Object(),extra=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return Symbol("result")};
            extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";throw "extra"};
            try{String.prototype.trimEnd.call(receiver,extra)}
            catch(error){return error.name+":"+error.message+"|"+log}
            return "missing";
        })()
// ERROR_CASES
// case: trim rejects a null receiver with the exact TypeError
String.prototype.trim.call(null)
// case: trimEnd rejects an undefined receiver with the exact TypeError
String.prototype.trimEnd.call(undefined)
// case: trimStart rejects a Symbol receiver with the exact TypeError
String.prototype.trimStart.call(Symbol('receiver'))
// case: trimRight alias rejects a null receiver
String.prototype.trimRight.call(null)
// case: trimLeft alias rejects an undefined receiver
String.prototype.trimLeft.call(undefined)
// case: receiver ToPrimitive returning an object is rejected
(function(){
            var receiver=Object();receiver[Symbol.toPrimitive]=function(){return Object()};
            return String.prototype.trim.call(receiver);
        })()
// case: trim is not a constructor
new String.prototype.trim()
// case: trimEnd is not a constructor
new String.prototype.trimEnd()
// case: trimRight alias is not a constructor
new String.prototype.trimRight()
// case: trimStart is not a constructor
new String.prototype.trimStart()
// case: trimLeft alias is not a constructor
new String.prototype.trimLeft()
// STACK_CASES
// case: recursive receiver conversion throws catchably and runtime recovers
(function(){
            var receiver=Object(),errorName="",errorMessage="";
            receiver[Symbol.toPrimitive]=function(){
                return String.prototype.trim.call(receiver)
            };
            try{String.prototype.trim.call(receiver)}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage," x ".trim()," x ".trimStart(),
                " x ".trimEnd()].join("|");
        })()
// case: trim aliases and existing String methods share one recursion guard
(function(){
            var value=Object(),depth=0,errorName="",errorMessage="";
            value[Symbol.toPrimitive]=function(){
                depth++;
                if(depth%8===0)return String.prototype.trim.call(value);
                if(depth%8===1)return String.prototype.trimEnd.call(value);
                if(depth%8===2)return String.prototype.trimRight.call(value);
                if(depth%8===3)return String.prototype.trimStart.call(value);
                if(depth%8===4)return "x".padEnd(value,"_");
                if(depth%8===5)return "x".repeat(value);
                if(depth%8===6)return "abcdef".slice(value,4);
                return "abcdef".includes("a",value);
            };
            try{String.prototype.trimLeft.call(value)}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage," x ".trim()," x ".trimRight(),
                " x ".trimLeft(),"x".padEnd(3,"_"),"ok".repeat(2),
                "abcdef".slice(1,3),"abcdef".includes("bc")].join("|");
        })()
