// prelude: regexp_match_all
// METADATA_CASES
// case: String matchAll exposes pinned metadata and prototype key order
(function(){
            var fn=String.prototype.matchAll,
                keys=Object.getOwnPropertyNames(String.prototype),
                selected=["startsWith","match","matchAll","search","split","substring"],
                order=[],index=0,key;
            while(index<keys.length){
                key=keys[index++];
                if(selected.indexOf(key)>=0)order[order.length]=key;
            }
            return [
                order.join(","),__bits(String.prototype,"matchAll"),
                fn.name,fn.length,Object.getOwnPropertyNames(fn).join(","),
                __bits(fn,"name"),__bits(fn,"length"),__isConstructor(fn),
                Object.prototype.hasOwnProperty.call(fn,"prototype")
            ].join("|");
        })()
// case: String matchAll AutoInit identity is stable and independently replaceable
(function(){
            var first=String.prototype.matchAll,
                second=String.prototype.matchAll,
                stable=first===second,
                deleted=delete String.prototype.matchAll;
            String.prototype.matchAll=17;
            return [stable,deleted,String.prototype.matchAll,
                __bits(String.prototype,"matchAll"),
                typeof first,first.name,first.length].join("|");
        })()
// DISPATCH_CASES
// case: String matchAll gets the hook then checks regexp identity flags and delegates raw this
(function(){
            var log="",receiver=Object(),regexp=Object(),flags=Object(),result=Object();
            receiver.toString=function(){log+="BAD-receiver-string;";throw "converted"};
            Object.defineProperty(regexp,Symbol.matchAll,{get:function(){
                log+="matchAll-get;";
                return function(value){
                    log+="matchAll-call:"+(this===regexp)+":"+(value===receiver)+":"+
                        arguments.length+";";
                    return result;
                };
            }});
            Object.defineProperty(regexp,Symbol.match,{get:function(){
                log+="match-get;";return true;
            }});
            Object.defineProperty(regexp,"flags",{get:function(){
                log+="flags-get;";return flags;
            }});
            flags.toString=function(){log+="flags-string;";return "ig"};
            var actual=String.prototype.matchAll.call(receiver,regexp);
            return (actual===result)+"|"+log;
        })()
// case: Symbol.match false bypasses the g check while true enforces it before hook call
(function(){
            function run(matchValue,flags){
                var log="",regexp=Object(),result=Object();
                Object.defineProperty(regexp,Symbol.matchAll,{get:function(){
                    log+="all-get;";
                    return function(){log+="all-call;";return result};
                }});
                Object.defineProperty(regexp,Symbol.match,{get:function(){
                    log+="match-get;";return matchValue;
                }});
                Object.defineProperty(regexp,"flags",{get:function(){
                    log+="flags-get;";return flags;
                }});
                var completion=__completion(function(){
                    return String.prototype.matchAll.call("x",regexp)===result;
                });
                return completion+":"+log;
            }
            var real=/a/;real[Symbol.match]=false;
            real[Symbol.matchAll]=function(){return 81};
            return [
                run(false,""),run(true,""),run(true,"g"),
                String.prototype.matchAll.call("a",real)
            ].join("|");
        })()
// case: nullish this is rejected before pattern hooks and noncallable hooks after the g check
(function(){
            var log="",poison=Object(),noncallable=Object();
            Object.defineProperty(poison,Symbol.matchAll,{get:function(){
                log+="BAD-all;";throw 70;
            }});
            var nullResult=__completion(function(){
                    return String.prototype.matchAll.call(null,poison);
                }),
                undefinedResult=__completion(function(){
                    return String.prototype.matchAll.call(undefined,poison);
                });
            Object.defineProperty(noncallable,Symbol.matchAll,{get:function(){
                log+="all-get;";return 1;
            }});
            Object.defineProperty(noncallable,Symbol.match,{get:function(){
                log+="match-get;";return true;
            }});
            Object.defineProperty(noncallable,"flags",{get:function(){
                log+="flags-get;";return "g";
            }});
            var invalid=__completion(function(){
                return String.prototype.matchAll.call("x",noncallable);
            });
            return [nullResult,undefinedResult,invalid,log].join("|");
        })()
// case: missing null and throwing flags fail before delegation or receiver conversion
(function(){
            function run(flags,throws){
                var log="",receiver=Object(),regexp=Object();
                receiver.toString=function(){log+="BAD-receiver;";return "x"};
                Object.defineProperty(regexp,Symbol.matchAll,{get:function(){
                    log+="all-get;";
                    return function(){log+="BAD-call;";return 1};
                }});
                Object.defineProperty(regexp,Symbol.match,{get:function(){
                    log+="match-get;";return true;
                }});
                Object.defineProperty(regexp,"flags",{get:function(){
                    log+="flags-get;";
                    if(throws)throw 72;
                    return flags;
                }});
                return __completion(function(){
                    return String.prototype.matchAll.call(receiver,regexp);
                })+":"+log;
            }
            var poison=Object();
            poison.toString=function(){throw 73};
            return [run(undefined,false),run(null,false),run(poison,false),
                run("g",true)].join("|");
        })()
// FALLBACK_CASES
// case: String matchAll fallback converts this then uses retained RegExp and dynamic protocol
(function(){
            var log="",intrinsic=RegExp,receiver=Object(),pattern=Object(),
                result=Object(),gets=0;
            receiver.toString=function(){log+="receiver-string;";return "subject"};
            Object.defineProperty(pattern,Symbol.matchAll,{get:function(){
                log+="pattern-all:"+(++gets)+";";
                return null;
            }});
            Object.defineProperty(pattern,Symbol.match,{get:function(){
                log+="pattern-match;";return false;
            }});
            pattern.toString=function(){log+="pattern-string;";return "sub"};
            Object.defineProperty(intrinsic.prototype,Symbol.matchAll,{
                configurable:true,get:function(){
                    log+="new-all;";
                    return function(value){
                        log+="new-call:"+
                            (Object.getPrototypeOf(this)===intrinsic.prototype)+":"+
                            (value==="subject")+":"+this.flags+":"+arguments.length+";";
                        return result;
                    };
                }
            });
            globalThis.RegExp=function(){log+="BAD-global;";throw "global constructor used"};
            var actual=String.prototype.matchAll.call(receiver,pattern);
            return (actual===result)+"|"+log;
        })()
// case: primitive patterns bypass boxed Symbol.matchAll hooks and become global regexps
(function(){
            var hits="",output=[];
            function poison(prototype,label){
                Object.defineProperty(prototype,Symbol.matchAll,{configurable:true,get:function(){
                    hits+=label;throw label;
                }});
            }
            poison(String.prototype,"string;");
            poison(Number.prototype,"number;");
            poison(Boolean.prototype,"boolean;");
            poison(BigInt.prototype,"bigint;");
            var stringIterator=String.prototype.matchAll.call("aba","a"),
                numberIterator=String.prototype.matchAll.call("a7b",7),
                booleanIterator=String.prototype.matchAll.call("atrueb",true),
                bigintIterator=String.prototype.matchAll.call("a9b",9n);
            output[0]=__step(stringIterator)+","+__step(stringIterator)+","+
                __step(stringIterator);
            output[1]=__step(numberIterator)+","+__step(numberIterator);
            output[2]=__step(booleanIterator)+","+__step(booleanIterator);
            output[3]=__step(bigintIterator)+","+__step(bigintIterator);
            return output.join("|")+"|hits:"+hits;
        })()
// case: native String matchAll preserves a global source lastIndex and clones its start
(function(){
            var regexp=/a/g;regexp.lastIndex=1;
            var iterator=String.prototype.matchAll.call("baac",regexp);
            return [
                __step(iterator),__step(iterator),__step(iterator),
                regexp.lastIndex,Object.getPrototypeOf(iterator)===
                    Object.getPrototypeOf(/x/g[Symbol.matchAll](""))
            ].join("|");
        })()
// case: null RegExp hook falls back through a fresh global clone starting at zero
(function(){
            var regexp=/a/g;regexp.lastIndex=2;
            Object.defineProperty(regexp,Symbol.matchAll,{value:null});
            var iterator=String.prototype.matchAll.call("baac",regexp);
            return [
                __step(iterator),__step(iterator),__step(iterator),
                regexp.lastIndex
            ].join("|");
        })()
// ERROR_CASES
// case: non-global RegExp rejection precedes hook call and receiver conversion
(function(){
            var log="",receiver=Object(),regexp=/a/;
            receiver.toString=function(){log+="BAD-receiver;";return "a"};
            Object.defineProperty(regexp,Symbol.matchAll,{get:function(){
                log+="all-get;";
                return function(){log+="BAD-call;";return 1};
            }});
            Object.defineProperty(regexp,"flags",{get:function(){
                log+="flags-get;";return "i";
            }});
            return __completion(function(){
                return String.prototype.matchAll.call(receiver,regexp);
            })+"|"+log;
        })()
// case: Symbol.matchAll and Symbol.match getter errors preserve exact short-circuit order
(function(){
            function allError(){
                var log="",regexp=Object();
                Object.defineProperty(regexp,Symbol.matchAll,{get:function(){
                    log+="all;";throw 71;
                }});
                Object.defineProperty(regexp,Symbol.match,{get:function(){
                    log+="BAD-match;";return true;
                }});
                return __completion(function(){
                    return String.prototype.matchAll.call("x",regexp);
                })+":"+log;
            }
            function matchError(){
                var log="",regexp=Object();
                Object.defineProperty(regexp,Symbol.matchAll,{get:function(){
                    log+="all;";return function(){log+="BAD-call;";return 1};
                }});
                Object.defineProperty(regexp,Symbol.match,{get:function(){
                    log+="match;";throw 72;
                }});
                Object.defineProperty(regexp,"flags",{get:function(){
                    log+="BAD-flags;";return "g";
                }});
                return __completion(function(){
                    return String.prototype.matchAll.call("x",regexp);
                })+":"+log;
            }
            return allError()+"|"+matchError();
        })()
// case: fallback input and pattern conversion errors occur before iterator creation
(function(){
            function inputError(){
                var log="",receiver=Object(),pattern=Object();
                receiver.toString=function(){log+="input;";throw 73};
                pattern[Symbol.matchAll]=null;
                pattern[Symbol.match]=false;
                pattern.toString=function(){log+="BAD-pattern;";return "x"};
                return __completion(function(){
                    return String.prototype.matchAll.call(receiver,pattern);
                })+":"+log;
            }
            function patternError(){
                var log="",receiver=Object(),pattern=Object();
                receiver.toString=function(){log+="input;";return "x"};
                pattern[Symbol.matchAll]=null;
                pattern[Symbol.match]=false;
                pattern.toString=function(){log+="pattern;";throw 74};
                return __completion(function(){
                    return String.prototype.matchAll.call(receiver,pattern);
                })+":"+log;
            }
            return inputError()+"|"+patternError();
        })()
