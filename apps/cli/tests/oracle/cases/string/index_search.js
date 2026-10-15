// VALUE_CASES
// case: upstream ordinary search matrix
(function(){
            return "abcabc".indexOf("cab")+"|"+"abcabc".indexOf("cab2")+"|"+
                "abc".indexOf("c")+"|"+"abcabc".lastIndexOf("abc")+"|"+
                "abcabc".lastIndexOf("missing")+"|"+"aaaa".indexOf("aa")+"|"+
                "aaaa".lastIndexOf("aa");
        })()
// case: indexOf ordinary and empty-needle position matrix
(function(){
            function row(needle){
                var positions=[undefined,0/0,-1/0,-1,-0,0,1,2,3,4,1/0,1.9,2.9,-1.9];
                var out=["aaa".indexOf(needle)];
                for(var i=0;i<positions.length;i++)
                    out[out.length]="aaa".indexOf(needle,positions[i]);
                return out.join(",");
            }
            return row("a")+"|"+row("");
        })()
// case: lastIndexOf ordinary and empty-needle position matrix
(function(){
            function row(needle){
                var positions=[undefined,0/0,-1/0,-1,-0,0,1,2,3,4,1/0,1.9,2.9,-1.9];
                var out=["aaa".lastIndexOf(needle)];
                for(var i=0;i<positions.length;i++)
                    out[out.length]="aaa".lastIndexOf(needle,positions[i]);
                return out.join(",");
            }
            return row("a")+"|"+row("");
        })()
// case: missing search and generic primitive receivers
(function(){
            var index="".indexOf,last="".lastIndexOf;
            return "undefined".indexOf()+"|"+"xundefinedx".lastIndexOf()+"|"+
                index.call(12323,"23")+"|"+last.call(true,"u")+"|"+
                index.call(717n,"17")+"|"+last.call(false,"false");
        })()
// case: UTF-16 code-unit and surrogate searches
(function(){
            var source="\ud83d\ude00\ud83dX\ude00";
            return source.length+"|"+source.indexOf("\ud83d")+"|"+
                source.lastIndexOf("\ud83d")+"|"+source.indexOf("\ude00")+"|"+
                source.lastIndexOf("\ude00")+"|"+source.indexOf("\ud83d\ude00")+"|"+
                source.lastIndexOf("\ud83d\ude00")+"|"+
                source.indexOf("\ude00\ud83d")+"|"+source.indexOf("\ude00",2)+"|"+
                source.lastIndexOf("\ud83d",1);
        })()
// case: rope source and needle cross 512 and 8192-code-unit boundaries
(function(){
            function grow(character,power){
                var value=character;
                for(var i=0;i<power;i++)value=value+value;
                return value;
            }
            var small=grow("a",9),large=grow("b",13);
            var source=small+"XYZ"+large+"Q"+small;
            var ropeNeedle=grow("b",9)+"Q";
            return source.length+"|"+source.indexOf("aXYZb")+"|"+
                source.lastIndexOf("bQa")+"|"+source.indexOf(ropeNeedle)+"|"+
                source.lastIndexOf("bb",8194)+"|"+source.indexOf("XYZ",512)+"|"+
                source.lastIndexOf("XYZ",511);
        })()
// ORDER_CASES
// case: receiver search and position conversions use string string number hints
(function(){
            function run(method){
                var log="",receiver=Object(),search=Object(),position=Object();
                receiver[Symbol.toPrimitive]=function(hint){log+="r:"+hint+",";return "abcabc"};
                search[Symbol.toPrimitive]=function(hint){log+="s:"+hint+",";return "bc"};
                position[Symbol.toPrimitive]=function(hint){log+="p:"+hint+",";return 1.9};
                return method.call(receiver,search,position)+":"+log;
            }
            return run("".indexOf)+"|"+run("".lastIndexOf);
        })()
// case: abrupt conversions stop later receiver search and position work
(function(){
            function run(method,stage){
                var log="",receiver=Object(),search=Object(),position=Object();
                receiver[Symbol.toPrimitive]=function(hint){
                    log+="r:"+hint+",";if(stage===0)throw "receiver";return "abcabc";
                };
                search[Symbol.toPrimitive]=function(hint){
                    log+="s:"+hint+",";if(stage===1)throw "search";return "bc";
                };
                position[Symbol.toPrimitive]=function(hint){
                    log+="p:"+hint+",";if(stage===2)throw "position";return 1;
                };
                try{return method.call(receiver,search,position)+":"+log}
                catch(error){return "throw:"+error+":"+log}
            }
            return run("".indexOf,0)+"|"+run("".indexOf,1)+"|"+
                run("".indexOf,2)+"|"+run("".lastIndexOf,0)+"|"+
                run("".lastIndexOf,1)+"|"+run("".lastIndexOf,2);
        })()
// case: omitted position skips conversion while explicit undefined is converted
(function(){
            function run(method){
                var log="",position=Object();
                position[Symbol.toPrimitive]=function(hint){log+=hint+",";return undefined};
                var omitted=method.call("aaa","a");
                var converted=method.call("aaa","a",position);
                return omitted+":"+converted+":"+log;
            }
            return run("".indexOf)+"|"+run("".lastIndexOf);
        })()
// case: index searches never inspect Symbol.match
(function(){
            function run(method){
                var log="",needle=Object();
                needle.__defineGetter__(Symbol.match,function(){log+="match,";throw "match"});
                needle.toString=function(){log+="string,";return "bc"};
                return method.call("abcabc",needle)+":"+log;
            }
            return run("".indexOf)+"|"+run("".lastIndexOf);
        })()
// ERROR_CASES
// case: indexOf rejects null receiver
(function(){var method="".indexOf;return method.call(null,"x")})()
// case: lastIndexOf rejects undefined receiver
(function(){var method="".lastIndexOf;return method.call(undefined,"x")})()
// case: indexOf rejects Symbol receiver
(function(){var method="".indexOf;return method.call(Symbol("receiver"),"x")})()
// case: indexOf rejects Symbol search
"x".indexOf(Symbol("search"))
// case: lastIndexOf rejects Symbol search
"x".lastIndexOf(Symbol("search"))
// case: indexOf rejects Symbol position
"x".indexOf("x",Symbol("position"))
// case: lastIndexOf rejects Symbol position
"x".lastIndexOf("x",Symbol("position"))
// case: indexOf rejects BigInt position
"x".indexOf("x",1n)
// case: lastIndexOf rejects BigInt position
"x".lastIndexOf("x",1n)
