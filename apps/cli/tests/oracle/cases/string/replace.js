// prelude: regexp_replace
// METADATA_CASES
// case: String replace methods expose pinned metadata descriptors and table order
(function(){
            var replace=String.prototype.replace,
                replaceAll=String.prototype.replaceAll,
                keys=Object.getOwnPropertyNames(String.prototype),
                selected=["slice","repeat","replace","replaceAll","padEnd","padStart","trim"],
                order=[],index=0,key;
            while(index<keys.length){
                key=keys[index++];
                if(selected.indexOf(key)>=0)order[order.length]=key;
            }
            return [
                order.join(","),
                __bits(String.prototype,"replace"),replace.name,replace.length,
                Object.getOwnPropertyNames(replace).join(","),
                __bits(replace,"name"),__bits(replace,"length"),
                __isConstructor(replace),
                Object.prototype.hasOwnProperty.call(replace,"prototype"),
                __bits(String.prototype,"replaceAll"),replaceAll.name,replaceAll.length,
                Object.getOwnPropertyNames(replaceAll).join(","),
                __bits(replaceAll,"name"),__bits(replaceAll,"length"),
                __isConstructor(replaceAll),
                Object.prototype.hasOwnProperty.call(replaceAll,"prototype"),
                replace===replaceAll
            ].join("|");
        })()
// case: String replace AutoInit entries retain identity and mutate independently
(function(){
            var replace=String.prototype.replace,
                replaceAll=String.prototype.replaceAll,
                stable=replace===String.prototype.replace&&
                    replaceAll===String.prototype.replaceAll,
                deleteReplace=delete String.prototype.replace,
                deleteReplaceAll=delete String.prototype.replaceAll;
            String.prototype.replace=17;
            String.prototype.replaceAll=23;
            return [stable,deleteReplace,deleteReplaceAll,
                String.prototype.replace,String.prototype.replaceAll,
                __bits(String.prototype,"replace"),
                __bits(String.prototype,"replaceAll"),
                replace===replaceAll].join("|");
        })()
// PROTOCOL_CASES
// case: replace and replaceAll intercept object protocols before any string conversion
(function(){
            function run(all){
                var log="",receiver=Object(),search=Object(),replacement=Object(),result=Object();
                receiver.toString=function(){log+="BAD-receiver;";throw "receiver"};
                replacement.toString=function(){log+="BAD-replacement;";throw "replacement"};
                if(all){
                    Object.defineProperty(search,Symbol.match,{get:function(){
                        log+="match-get;";return false;
                    }});
                }
                Object.defineProperty(search,Symbol.replace,{get:function(){
                    log+="replace-get;";
                    return function(input,value){
                        log+="replace-call:"+(this===search)+":"+(input===receiver)+":"+
                            (value===replacement)+":"+arguments.length+";";
                        return result;
                    };
                }});
                var actual=all?
                    String.prototype.replaceAll.call(receiver,search,replacement):
                    String.prototype.replace.call(receiver,search,replacement);
                return (actual===result)+":"+log;
            }
            return run(false)+"|"+run(true);
        })()
// case: replaceAll validates IsRegExp and global flags before reading Symbol.replace
(function(){
            function run(matchValue,flagsValue){
                var log="",search=Object(),receiver=Object(),replacement=Object();
                Object.defineProperty(search,Symbol.match,{get:function(){
                    log+="match-get;";return matchValue;
                }});
                Object.defineProperty(search,"flags",{get:function(){
                    log+="flags-get;";return flagsValue;
                }});
                Object.defineProperty(search,Symbol.replace,{get:function(){
                    log+="replace-get;";
                    return function(input,value){
                        log+="replace-call:"+(input===receiver)+":"+
                            (value===replacement)+";";
                        return "hook";
                    };
                }});
                if(flagsValue!==null&&flagsValue!==undefined&&typeof flagsValue==="object")
                    flagsValue.toString=function(){log+="flags-string;";return this.value};
                var completion=__completion(function(){
                    return String.prototype.replaceAll.call(receiver,search,replacement);
                });
                return completion+":"+log;
            }
            var globalFlags=Object();globalFlags.value="ug",
                localFlags=Object();localFlags.value="u";
            return [
                run(true,globalFlags),
                run(true,localFlags),
                run(true,null),
                run(false,localFlags)
            ].join("|");
        })()
// case: primitive search values bypass boxed hooks and invalid hooks precede receiver conversion
(function(){
            var hits="",log="",receiver=Object(),invalid=Object(),poison=Object();
            Object.defineProperty(Number.prototype,Symbol.replace,{configurable:true,get:function(){
                hits+="number;";throw "number hook";
            }});
            Object.defineProperty(String.prototype,Symbol.replace,{configurable:true,get:function(){
                hits+="string;";throw "string hook";
            }});
            receiver.toString=function(){log+="BAD-receiver;";return "a1b"};
            Object.defineProperty(invalid,Symbol.replace,{get:function(){
                log+="invalid-get;";return 17;
            }});
            Object.defineProperty(poison,Symbol.replace,{get:function(){
                log+="BAD-nullish;";return function(){};
            }});
            return [
                String.prototype.replace.call("a1b1",1,"X"),
                String.prototype.replaceAll.call("a,b,c",",","X"),
                __completion(function(){
                    return String.prototype.replace.call(receiver,invalid,"X");
                }),
                __completion(function(){
                    return String.prototype.replace.call(null,poison,"X");
                }),
                hits,log
            ].join("|");
        })()
// CONVERSION_CASES
// case: string fallback converts receiver search and noncallable replacement before searching
(function(){
            function run(all,needle){
                var log="",receiver=Object(),search=Object(),replacement=Object();
                receiver.toString=function(){log+="receiver;";return "abcabc"};
                search.toString=function(){log+="search;";return needle};
                replacement.toString=function(){log+="replacement;";return "X"};
                var result=all?
                    String.prototype.replaceAll.call(receiver,search,replacement):
                    String.prototype.replace.call(receiver,search,replacement);
                return result+":"+log;
            }
            return [run(false,"z"),run(true,"b"),run(true,"z")].join("|");
        })()
// case: functional replacements skip upfront ToString and receive exact arguments
(function(){
            function run(all,needle){
                var log="",receiver=Object(),search=Object(),calls=0;
                receiver.toString=function(){log+="receiver;";return "aba"};
                search.toString=function(){log+="search;";return needle};
                function replacement(match,position,input){
                    "use strict";
                    calls++;
                    log+="call:"+(this===undefined)+":"+match+":"+position+":"+
                        input+":"+arguments.length+";";
                    var result=Object();
                    result.toString=function(){log+="result-string;";return "X"};
                    return result;
                }
                replacement.toString=function(){log+="BAD-function-string;";throw "function"};
                var result=all?
                    String.prototype.replaceAll.call(receiver,search,replacement):
                    String.prototype.replace.call(receiver,search,replacement);
                return result+":"+calls+":"+log;
            }
            return [run(false,"a"),run(true,"a"),run(true,"z")].join("|");
        })()
// case: fallback abrupt completions stop at receiver search and replacement boundaries
(function(){
            function run(stage){
                var log="",sentinel=Object(),receiver=Object(),search=Object(),replacement=Object(),
                    same=false;
                receiver.toString=function(){log+="receiver;";if(stage===0)throw sentinel;return "abc"};
                search.toString=function(){log+="search;";if(stage===1)throw sentinel;return "z"};
                replacement.toString=function(){log+="replacement;";if(stage===2)throw sentinel;return "X"};
                try{String.prototype.replace.call(receiver,search,replacement)}
                catch(error){same=error===sentinel}
                return same+":"+log;
            }
            return [run(0),run(1),run(2)].join("|");
        })()
// UTF16_AND_SUBSTITUTION_CASES
// case: string replacement expands every substitution token without captures
(function(){
            var template="$$|$&|$`|$'|$0|$1|$01|$99|$<x>|$<";
            return [
                "abc".replace("b",template),
                "abc".replaceAll("b",template),
                "aaa".replace("a","X"),
                "aaa".replaceAll("a","X")
            ].join("|");
        })()
// case: replaceAll empty search walks every UTF-16 code-unit boundary
(function(){
            var input=String.fromCharCode(0xd83d,0xde00,0xd800),
                positions=[],matches=[],inputs=[],calls=0;
            var result=input.replaceAll("",function(match,position,source){
                calls++;positions[positions.length]=position;
                matches[matches.length]=match.length;
                inputs[inputs.length]=source===input;
                return "|";
            });
            return [__units(input),__units(result),calls,positions.join(","),
                matches.join(","),inputs.join(",")].join("|");
        })()
// case: empty replacement tokens use code-unit positions including both string ends
(function(){
            var input=String.fromCharCode(0xd83d,0xde00);
            return [
                __units(input.replaceAll("","[$`][$&][$']")),
                __units("".replaceAll("","X")),
                __units("".replace("","$`-$&-$'"))
            ].join("|");
        })()
