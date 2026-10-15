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
function __show(value){
    if(value===undefined)return "undefined";
    if(value===null)return "null";
    if(typeof value==="number"){
        if(value!==value)return "NaN";
        if(value===0)return 1/value===-Infinity?"-0":"+0";
    }
    return String(value);
}
function __completion(callback){
    try{return "return:"+String(callback())}
    catch(error){
        if(error!==null&&typeof error==="object")
            return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
function __array(value){
    var output=[],index=0;
    while(index<value.length){
        output[index]=(index in value)?__show(value[index]):"<hole>";
        index++;
    }
    return value.length+"["+output.join(",")+"]";
}
// METADATA_CASES
// case: split methods expose pinned metadata descriptors and filtered key order
(function(){
            var stringFn=String.prototype.split,
                regexpFn=RegExp.prototype[Symbol.split],
                stringKeys=Object.getOwnPropertyNames(String.prototype),
                regexpKeys=Reflect.ownKeys(RegExp.prototype),
                selected=["startsWith","search","split","substring"],
                stringOrder=[],regexpOrder=[],index=0,key;
            while(index<stringKeys.length){
                key=stringKeys[index];
                if(selected.indexOf(key)>=0)stringOrder[stringOrder.length]=key;
                index++;
            }
            index=0;
            while(index<regexpKeys.length){
                key=regexpKeys[index];
                if(key==="test"||key==="toString"||key===Symbol.match||
                    key===Symbol.search||key===Symbol.split)
                    regexpOrder[regexpOrder.length]=String(key);
                index++;
            }
            return [
                stringOrder.join(","),regexpOrder.join(","),
                __bits(String.prototype,"split"),stringFn.name,stringFn.length,
                Object.getOwnPropertyNames(stringFn).join(","),
                __bits(stringFn,"name"),__bits(stringFn,"length"),
                __isConstructor(stringFn),
                Object.prototype.hasOwnProperty.call(stringFn,"prototype"),
                __bits(RegExp.prototype,Symbol.split),regexpFn.name,regexpFn.length,
                Object.getOwnPropertyNames(regexpFn).join(","),
                __bits(regexpFn,"name"),__bits(regexpFn,"length"),
                __isConstructor(regexpFn),
                Object.prototype.hasOwnProperty.call(regexpFn,"prototype")
            ].join("|");
        })()
// case: split AutoInit entries retain identity and can be replaced independently
(function(){
            var stringOriginal=String.prototype.split,
                regexpOriginal=RegExp.prototype[Symbol.split],
                stable=stringOriginal===String.prototype.split&&
                    regexpOriginal===RegExp.prototype[Symbol.split],
                stringDeleted=delete String.prototype.split,
                regexpDeleted=delete RegExp.prototype[Symbol.split];
            String.prototype.split=17;
            RegExp.prototype[Symbol.split]=23;
            return [stable,stringDeleted,regexpDeleted,String.prototype.split,
                RegExp.prototype[Symbol.split],__bits(String.prototype,"split"),
                __bits(RegExp.prototype,Symbol.split),
                stringOriginal===regexpOriginal].join("|");
        })()
// STRING_INTEGRATION_CASES
// case: String split activates the inherited RegExp Symbol.split protocol
(function(){
            var first=/(-)/g,second=/b/;
            first.lastIndex=7;second.lastIndex=9;
            var left="a-b-c".split(first,4),right=String.prototype.split.call("abc",second);
            return [__array(left),first.lastIndex,__array(right),second.lastIndex,
                Object.getPrototypeOf(left)===Array.prototype,
                Object.getPrototypeOf(right)===Array.prototype].join("|");
        })()
// case: String split gets Symbol.split before RegExp input conversion and passes raw limit
(function(){
            var log="",receiver=Object(),limit=Object(),regexp=/b/,
                builtin=RegExp.prototype[Symbol.split];
            receiver.toString=function(){log+="input-string;";return "abc"};
            limit.valueOf=function(){log+="limit-number;";return 2};
            Object.defineProperty(regexp,Symbol.split,{get:function(){
                log+="split-get;";
                return function(input,rawLimit){
                    log+="split-call:"+(this===regexp)+":"+(input===receiver)+":"+
                        (rawLimit===limit)+";";
                    return builtin.call(this,input,rawLimit);
                };
            }});
            return __array(String.prototype.split.call(receiver,regexp,limit))+"|"+log;
        })()
// SPECIES_ORDER_CASES
// case: RegExp split observes input constructor species flags construct limit and exec order
(function(){
            var log="",regexp=Object(),holder=Object(),input=Object(),flags=Object(),limit=Object();
            input.toString=function(){log+="input-string;";return "x"};
            Object.defineProperty(regexp,"constructor",{get:function(){
                log+="constructor-get;";return holder;
            }});
            Object.defineProperty(holder,Symbol.species,{get:function(){
                log+="species-get:"+(this===holder)+";";return Species;
            }});
            Object.defineProperty(regexp,"flags",{get:function(){log+="flags-get;";return flags}});
            flags.toString=function(){log+="flags-string;";return "g"};
            function Species(pattern,observedFlags){
                log+="construct:"+(new.target===Species)+":"+(pattern===regexp)+":"+
                    observedFlags+":"+arguments.length+";";
                var splitter=Object();splitter.lastIndex=0;
                splitter.exec=function(text){log+="exec:"+(this===splitter)+":"+text+";";return null};
                return splitter;
            }
            limit.valueOf=function(){log+="limit-number;";return 3};
            var result=RegExp.prototype[Symbol.split].call(regexp,input,limit);
            return __array(result)+"|"+log;
        })()
// case: primitive constructor and nonconstructor species are TypeErrors before flags
(function(){
            function primitive(){
                var log="",regexp=Object();
                Object.defineProperty(regexp,"constructor",{get:function(){log+="constructor;";return 1}});
                Object.defineProperty(regexp,"flags",{get:function(){log+="BAD-flags;";return ""}});
                var completion=__completion(function(){
                    return RegExp.prototype[Symbol.split].call(regexp,"x");
                });
                return completion+":"+log;
            }
            function species(value){
                var log="",regexp=Object(),holder=Object();
                regexp.constructor=holder;
                Object.defineProperty(holder,Symbol.species,{get:function(){log+="species;";return value}});
                Object.defineProperty(regexp,"flags",{get:function(){log+="BAD-flags;";return ""}});
                var completion=__completion(function(){
                    return RegExp.prototype[Symbol.split].call(regexp,"x");
                });
                return completion+":"+log;
            }
            return [primitive(),species(1),species(RegExp.prototype.exec)].join("|");
        })()
// case: nullish species uses retained RegExp and ignores poisoned global constructors
(function(){
            var intrinsicRegExp=RegExp,intrinsicArray=Array,split=RegExp.prototype[Symbol.split],
                first=/b/,second=/b/,firstHolder=Object(),secondHolder=Object();
            firstHolder[Symbol.species]=null;secondHolder[Symbol.species]=undefined;
            first.constructor=firstHolder;second.constructor=secondHolder;
            globalThis.RegExp=function(){throw "global RegExp used"};
            globalThis.Array=function(){throw "global Array used"};
            var left=split.call(first,"abc"),right=split.call(second,"abc");
            return [__array(left),__array(right),
                Object.getPrototypeOf(left)===intrinsicArray.prototype,
                Object.getPrototypeOf(right)===intrinsicArray.prototype,
                Object.getPrototypeOf(first)===intrinsicRegExp.prototype].join("|");
        })()
// ERROR_CASES
// case: RegExp Symbol.split rejects primitive receivers before input conversion
(function(){
            var input=Object(),hits=0;
            input.toString=function(){hits++;return "x"};
            var completion=__completion(function(){
                return RegExp.prototype[Symbol.split].call(1,input);
            });
            return completion+"|"+hits;
        })()
// FLAGS_AND_ADVANCE_CASES
// case: split appends sticky once and uses u or v for surrogate-pair advancement
(function(){
            function run(flags){
                var regexp=Object(),holder=Object(),sets="",seenFlags="";
                function Species(_pattern,constructedFlags){
                    seenFlags=constructedFlags;
                    var state=0,splitter=Object();
                    Object.defineProperty(splitter,"lastIndex",{
                        get:function(){return state},
                        set:function(value){if(sets!=="")sets+=",";sets+=value;state=value}
                    });
                    splitter.exec=function(){return null};
                    return splitter;
                }
                holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags=flags;
                var result=RegExp.prototype[Symbol.split].call(
                    regexp,String.fromCharCode(0xd83d,0xde00,0x58));
                return seenFlags+":"+sets+":"+result.length+":"+result[0].length;
            }
            return [run(""),run("y"),run("u"),run("uy"),run("v")].join("|");
        })()
// LIMIT_AND_EMPTY_CASES
// case: real RegExp split honors undefined zero Uint32 wrapping and capture truncation
(function(){
            return [
                __array("ab12cd34".split(/\d+/,undefined)),
                __array("ab12cd34".split(/\d+/,0)),
                __array("a1b2".split(/(\d)/,3)),
                __array("a-b-c".split(/-/,4294967297)),
                __array("a-b-c".split(/-/,-1))
            ].join("|");
        })()
// case: zero limit still constructs the species but never executes it
(function(){
            var log="",regexp=Object(),holder=Object(),limit=Object();
            function Species(){
                log+="construct;";
                return {lastIndex:0,exec:function(){log+="BAD-exec;";return null}};
            }
            holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
            limit.valueOf=function(){log+="limit;";return 0};
            return __array(RegExp.prototype[Symbol.split].call(regexp,"abc",limit))+"|"+log;
        })()
// case: empty input executes once without setting lastIndex and distinguishes hit from miss
(function(){
            function run(hit){
                var regexp=Object(),holder=Object(),splitter,calls=0;
                function Species(){
                    splitter=Object();
                    splitter.exec=function(value){calls++;return hit?{0:"",length:1}:null};
                    return splitter;
                }
                holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
                var result=RegExp.prototype[Symbol.split].call(regexp,"");
                return __array(result)+":"+calls+":"+
                    Object.prototype.hasOwnProperty.call(splitter,"lastIndex");
            }
            return run(false)+"|"+run(true)+"|"+__array("".split(/(?:)/));
        })()
// CAPTURE_CASES
// case: capture values remain raw and are read only through the active limit
(function(){
            function run(limit){
                var log="",capture=Object(),regexp=Object(),holder=Object(),calls=0;
                function Species(){
                    var splitter=Object();splitter.lastIndex=0;
                    splitter.exec=function(){
                        calls++;
                        if(calls!==1)return null;
                        this.lastIndex=2;
                        var result=Object();
                        Object.defineProperty(result,"length",{get:function(){log+="length;";return 4}});
                        Object.defineProperty(result,"1",{get:function(){log+="one;";return capture}});
                        Object.defineProperty(result,"2",{get:function(){log+="two;";return undefined}});
                        Object.defineProperty(result,"3",{get:function(){log+="three;";return false}});
                        return result;
                    };
                    return splitter;
                }
                holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
                var result=RegExp.prototype[Symbol.split].call(regexp,"abcd",limit);
                return [result.length,result[0],result[1]===capture,result[2]===undefined,
                    result[3]===false,result[4],log,calls].join(":");
            }
            return run(undefined)+"|"+run(2);
        })()
// LAST_INDEX_AND_EXEC_CASES
// case: a genuine RegExp receiver still calls an overridden prototype exec
(function(){
            var exec=RegExp.prototype.exec;
            RegExp.prototype.exec=function(){this.lastIndex=100;return {length:0,index:0}};
            try{return __array(/a/[Symbol.split]("foo"))}
            finally{RegExp.prototype.exec=exec}
        })()
// case: a genuine RegExp receiver still reads an @@match accessor for the splitter
(function(){
            var reads=0,regexp=/a/;
            Object.defineProperty(regexp,Symbol.match,{get:function(){reads++;regexp.compile("b")}});
            return __array(regexp[Symbol.split]("abba"))+":"+reads;
        })()
// case: empty matches advance and a retreating lastIndex is consumed exactly
(function(){
            function empty(){
                var regexp=Object(),holder=Object(),sets="";
                function Species(){
                    var state=0,splitter=Object();
                    Object.defineProperty(splitter,"lastIndex",{
                        get:function(){return state},
                        set:function(value){if(sets!=="")sets+=",";sets+=value;state=value}
                    });
                    splitter.exec=function(){return {length:1}};
                    return splitter;
                }
                holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
                return __array(RegExp.prototype[Symbol.split].call(regexp,"ab"))+":"+sets;
            }
            function retreat(){
                var regexp=Object(),holder=Object(),sets="",calls=0;
                function Species(){
                    var state=0,splitter=Object();
                    Object.defineProperty(splitter,"lastIndex",{
                        get:function(){return state},
                        set:function(value){if(sets!=="")sets+=",";sets+=value;state=value}
                    });
                    splitter.exec=function(){
                        calls++;
                        if(calls===1){state=2;return {length:1}}
                        if(calls===2){state=1;return {length:1}}
                        return null;
                    };
                    return splitter;
                }
                holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
                return __array(RegExp.prototype[Symbol.split].call(regexp,"abcd"))+":"+sets+":"+calls;
            }
            return empty()+"|"+retreat();
        })()
// case: lastIndex is clamped to input length after a successful abstract exec
(function(){
            var regexp=Object(),holder=Object(),calls=0;
            function Species(){
                var splitter=Object();splitter.lastIndex=0;
                splitter.exec=function(){calls++;this.lastIndex=99;return {length:1}};
                return splitter;
            }
            holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
            return __array(RegExp.prototype[Symbol.split].call(regexp,"abcd"))+"|"+calls;
        })()
// case: abstract exec accepts branded fallback and rejects missing or primitive results
(function(){
            function run(kind){
                var regexp=Object(),holder=Object();
                function Species(){
                    if(kind===0){var branded=/b/y;branded.exec=null;return branded}
                    if(kind===1)return {lastIndex:0,exec:null};
                    return {lastIndex:0,exec:function(){return 7}};
                }
                holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
                return kind===0?__array(RegExp.prototype[Symbol.split].call(regexp,"abc")):
                    __completion(function(){return RegExp.prototype[Symbol.split].call(regexp,"abc")});
            }
            return [run(0),run(1),run(2)].join("|");
        })()
// ABRUPT_CASES
// case: prefix abrupt completions stop at each input species flags construct and limit boundary
(function(){
            function run(stage){
                var log="",sentinel=Object(),regexp=Object(),holder=Object(),
                    input=Object(),flags=Object(),limit=Object(),same=false;
                input.toString=function(){log+="input;";if(stage===0)throw sentinel;return "x"};
                Object.defineProperty(regexp,"constructor",{get:function(){
                    log+="constructor;";if(stage===1)throw sentinel;return holder;
                }});
                Object.defineProperty(holder,Symbol.species,{get:function(){
                    log+="species;";if(stage===2)throw sentinel;return Species;
                }});
                Object.defineProperty(regexp,"flags",{get:function(){
                    log+="flags-get;";if(stage===3)throw sentinel;return flags;
                }});
                flags.toString=function(){log+="flags-string;";if(stage===4)throw sentinel;return ""};
                function Species(){
                    log+="construct;";if(stage===5)throw sentinel;
                    return {lastIndex:0,exec:function(){log+="exec;";return null}};
                }
                limit.valueOf=function(){log+="limit;";if(stage===6)throw sentinel;return 2};
                try{RegExp.prototype[Symbol.split].call(regexp,input,limit)}
                catch(error){same=error===sentinel}
                return same+":"+log;
            }
            var output=[],index=0;
            while(index<7){output[index]=run(index);index++}
            return output.join("|");
        })()
// case: loop abrupt completions stop at setter exec end length and capture boundaries
(function(){
            function run(stage){
                var log="",sentinel=Object(),regexp=Object(),holder=Object(),same=false;
                function Species(){
                    var state=0,splitter=Object(),result=Object(),end=Object(),length=Object();
                    end.valueOf=function(){log+="end-value;";if(stage===4)throw sentinel;return 1};
                    length.valueOf=function(){log+="length-value;";if(stage===6)throw sentinel;return 2};
                    Object.defineProperty(splitter,"lastIndex",{
                        get:function(){log+="last-get;";if(stage===3)throw sentinel;
                            return stage===4?end:state},
                        set:function(value){log+="last-set;";if(stage===0)throw sentinel;state=value}
                    });
                    Object.defineProperty(result,"length",{get:function(){
                        log+="length-get;";if(stage===5)throw sentinel;return stage===6?length:2;
                    }});
                    Object.defineProperty(result,"1",{get:function(){
                        log+="capture-get;";if(stage===7)throw sentinel;return "capture";
                    }});
                    Object.defineProperty(splitter,"exec",{get:function(){
                        log+="exec-get;";if(stage===1)throw sentinel;
                        return function(){log+="exec-call;";if(stage===2)throw sentinel;
                            state=1;return result};
                    }});
                    return splitter;
                }
                holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";
                try{RegExp.prototype[Symbol.split].call(regexp,"x")}
                catch(error){same=error===sentinel}
                return same+":"+log;
            }
            var output=[],index=0;
            while(index<8){output[index]=run(index);index++}
            return output.join("|");
        })()
// RECURSION_CASES
// case: recursive String and RegExp split protocol calls are catchable and recover
(function(){
            var separator=Object(),stringError="",regexp=Object(),holder=Object(),regexpError="";
            separator[Symbol.split]=function(value,limit){
                return String.prototype.split.call(value,separator,limit);
            };
            try{"x".split(separator)}catch(error){stringError=error.name+":"+error.message}
            function Species(){return regexp}
            holder[Symbol.species]=Species;regexp.constructor=holder;regexp.flags="";regexp.lastIndex=0;
            regexp.exec=function(){return RegExp.prototype[Symbol.split].call(regexp,"x")};
            try{RegExp.prototype[Symbol.split].call(regexp,"x")}
            catch(error){regexpError=error.name+":"+error.message}
            return [stringError,regexpError,__array("a-b".split(/-/)),
                __array((function(){
                    var recovery=Object(),recoveryHolder=Object();
                    recoveryHolder[Symbol.species]=function(){
                        return {lastIndex:0,exec:function(){return null}};
                    };
                    recovery.constructor=recoveryHolder;recovery.flags="";
                    return RegExp.prototype[Symbol.split].call(recovery,"x");
                })())].join("|");
        })()
