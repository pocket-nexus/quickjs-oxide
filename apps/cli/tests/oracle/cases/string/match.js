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
    if(value===null)return "null";
    var output=[],index=0;
    while(index<value.length){output[index]=String(value[index]);index++}
    return value.length+"["+output.join(",")+"]";
}
// METADATA_CASES
// case: match methods expose pinned metadata descriptors and filtered key order
(function(){
            var stringFn=String.prototype.match,
                regexpFn=RegExp.prototype[Symbol.match],
                stringKeys=Object.getOwnPropertyNames(String.prototype),
                regexpKeys=Reflect.ownKeys(RegExp.prototype),
                selected=["startsWith","match","search","split"],
                stringOrder=[],regexpOrder=[],index=0,key;
            while(index<stringKeys.length){
                key=stringKeys[index];
                if(selected.indexOf(key)>=0)stringOrder[stringOrder.length]=key;
                index++;
            }
            index=0;
            while(index<regexpKeys.length){
                key=regexpKeys[index];
                if(key==="test"||key==="toString"||key===Symbol.match||key===Symbol.search)
                    regexpOrder[regexpOrder.length]=String(key);
                index++;
            }
            return [
                stringOrder.join(","),regexpOrder.join(","),
                __bits(String.prototype,"match"),stringFn.name,stringFn.length,
                Object.getOwnPropertyNames(stringFn).join(","),
                __bits(stringFn,"name"),__bits(stringFn,"length"),
                __isConstructor(stringFn),
                Object.prototype.hasOwnProperty.call(stringFn,"prototype"),
                __bits(RegExp.prototype,Symbol.match),regexpFn.name,regexpFn.length,
                Object.getOwnPropertyNames(regexpFn).join(","),
                __bits(regexpFn,"name"),__bits(regexpFn,"length"),
                __isConstructor(regexpFn),
                Object.prototype.hasOwnProperty.call(regexpFn,"prototype")
            ].join("|");
        })()
// case: match AutoInit entries can be replaced deleted and materialized independently
(function(){
            var original=String.prototype.match,
                regexpOriginal=RegExp.prototype[Symbol.match],
                stable=original===String.prototype.match&&
                    regexpOriginal===RegExp.prototype[Symbol.match],
                stringDeleted=delete String.prototype.match,
                regexpDeleted=delete RegExp.prototype[Symbol.match];
            String.prototype.match=17;
            RegExp.prototype[Symbol.match]=23;
            return [stable,stringDeleted,regexpDeleted,String.prototype.match,
                RegExp.prototype[Symbol.match],__bits(String.prototype,"match"),
                __bits(RegExp.prototype,Symbol.match),original===regexpOriginal].join("|");
        })()
// STRING_DISPATCH_CASES
// case: String match delegates before conversion and returns the custom result unchanged
(function(){
            var log="",receiver=Object(),regexp=Object(),result=Object();
            receiver.toString=function(){log+="BAD-receiver-string;";throw "converted"};
            Object.defineProperty(regexp,Symbol.match,{get:function(){
                log+="get;";
                return function(value){
                    log+="call:"+(this===regexp)+":"+(value===receiver)+":"+
                        arguments.length+";";
                    return result;
                };
            }});
            var actual=String.prototype.match.call(receiver,regexp);
            return (actual===result)+"|"+log;
        })()
// case: String match rejects nullish and noncallable hooks before conversion
(function(){
            var log="",poison=Object(),receiver=Object(),noncallable=Object();
            Object.defineProperty(poison,Symbol.match,{get:function(){log+="BAD-nullish;";return null}});
            var first=__completion(function(){return String.prototype.match.call(null,poison)}),
                second=__completion(function(){return String.prototype.match.call(undefined,poison)});
            receiver.toString=function(){log+="BAD-string;";return "a"};
            Object.defineProperty(noncallable,Symbol.match,{get:function(){log+="hook-get;";return 17}});
            var invalid=__completion(function(){return String.prototype.match.call(receiver,noncallable)});
            return [first,second,invalid,log].join("|");
        })()
// case: primitive patterns bypass boxed prototype Symbol.match hooks
(function(){
            var hits="",output=[];
            function poison(prototype,label){
                Object.defineProperty(prototype,Symbol.match,{configurable:true,get:function(){
                    hits+=label;throw label;
                }});
            }
            poison(String.prototype,"string;");
            poison(Number.prototype,"number;");
            poison(Boolean.prototype,"boolean;");
            poison(BigInt.prototype,"bigint;");
            output[0]=__array(String.prototype.match.call("a7b",7));
            output[1]=__array(String.prototype.match.call("atrueb",true));
            output[2]=__array(String.prototype.match.call("a9b",9n));
            return output.join("|")+"|hits:"+hits;
        })()
// STRING_FALLBACK_CASES
// case: String match uses the retained intrinsic constructor and dynamic new protocol
(function(){
            var log="",intrinsic=RegExp,receiver=Object(),pattern=Object(),
                source=Object(),flags=Object(),matchGets=0;
            Object.defineProperty(pattern,Symbol.match,{get:function(){
                log+="pattern-match:"+(++matchGets)+";";
                return matchGets===1?null:true;
            }});
            Object.defineProperty(pattern,"source",{get:function(){log+="source-get;";return source}});
            Object.defineProperty(pattern,"flags",{get:function(){log+="flags-get;";return flags}});
            Object.defineProperty(pattern,"constructor",{get:function(){log+="BAD-constructor;";return intrinsic}});
            receiver.toString=function(){log+="receiver-string;";return "subject"};
            source.toString=function(){log+="source-string;";return "sub"};
            flags.toString=function(){log+="flags-string;";return ""};
            Object.defineProperty(intrinsic.prototype,Symbol.match,{configurable:true,get:function(){
                log+="new-match;";
                return function(value){
                    log+="new-call:"+(Object.getPrototypeOf(this)===intrinsic.prototype)+":"+
                        (value==="subject")+":"+__show(this.lastIndex)+";";
                    return 73;
                };
            }});
            globalThis.RegExp=function(){log+="BAD-global;";throw "global constructor used"};
            var result=String.prototype.match.call(receiver,pattern);
            return result+"|"+log;
        })()
// case: String match null and undefined hooks fall back after the receiver conversion
(function(){
            function run(hook){
                var log="",receiver=Object(),pattern=Object();
                receiver.toString=function(){log+="receiver;";return "abc"};
                Object.defineProperty(pattern,Symbol.match,{get:function(){log+="get;";return hook}});
                pattern.toString=function(){log+="pattern;";return "b"};
                return __array(String.prototype.match.call(receiver,pattern))+":"+log;
            }
            return run(null)+"|"+run(undefined);
        })()
// NON_GLOBAL_CASES
// case: RegExp Symbol.match converts input and flags before one abstract exec and returns raw result
(function(){
            var log="",input=Object(),flags=Object(),receiver=Object(),result=Object();
            input.toString=function(){log+="input-string;";return "abc"};
            Object.defineProperty(receiver,"flags",{get:function(){log+="flags-get;";return flags}});
            flags.toString=function(){log+="flags-string;";return "i"};
            Object.defineProperty(receiver,"exec",{get:function(){
                log+="exec-get;";
                return function(value){
                    log+="exec-call:"+(this===receiver)+":"+(value==="abc")+":"+
                        arguments.length+";";
                    return result;
                };
            }});
            var actual=RegExp.prototype[Symbol.match].call(receiver,input);
            return (actual===result)+"|"+log+"|has-lastIndex:"+
                Object.prototype.hasOwnProperty.call(receiver,"lastIndex");
        })()
// case: RegExp Symbol.match validates receiver before input and enforces abstract exec results
(function(){
            var hits=0,input=Object();
            input.toString=function(){hits++;throw "BAD-input"};
            var primitive=__completion(function(){
                return RegExp.prototype[Symbol.match].call(1,input);
            });
            var ordinary=Object();ordinary.flags=undefined;ordinary.exec=null;
            var nonregexp=__completion(function(){
                return RegExp.prototype[Symbol.match].call(ordinary,"x");
            });
            var invalid=Object();invalid.flags="";invalid.exec=function(){return 7};
            var invalidResult=__completion(function(){
                return RegExp.prototype[Symbol.match].call(invalid,"x");
            });
            var miss=Object();miss.flags="";miss.exec=function(){return null};
            return [primitive,hits,nonregexp,invalidResult,
                RegExp.prototype[Symbol.match].call(miss,"x")].join("|");
        })()
// case: RegExp Symbol.match builtin fallback accepts branded regexps and preserves non-global lastIndex
(function(){
            var branded=/b/;branded.lastIndex=2;branded.exec=null;
            var actual=RegExp.prototype[Symbol.match].call(branded,"abc");
            return [__array(actual),actual.index,actual.input,branded.lastIndex].join("|");
        })()
// GLOBAL_CASES
// case: global match resets lastIndex and collects stringified zero properties in a fresh Array
(function(){
            var log="",state=9,calls=0,receiver=Object(),first=Object(),second=Object(),text=Object();
            Object.defineProperty(receiver,"flags",{get:function(){log+="flags-get;";return "g"}});
            Object.defineProperty(receiver,"lastIndex",{
                get:function(){log+="last-get:"+__show(state)+";";return state},
                set:function(value){log+="last-set:"+__show(value)+";";state=value}
            });
            text.toString=function(){log+="zero-string;";return "A"};
            Object.defineProperty(first,"0",{get:function(){log+="zero-get-1;";return text}});
            Object.defineProperty(second,"0",{get:function(){log+="zero-get-2;";return 7}});
            Object.defineProperty(receiver,"exec",{get:function(){
                log+="exec-get;";
                return function(value){
                    calls++;log+="exec-call:"+calls+":"+value+";";
                    if(calls===1)return first;
                    if(calls===2)return second;
                    return null;
                };
            }});
            var actual=RegExp.prototype[Symbol.match].call(receiver,"subject"),
                zero=Object.getOwnPropertyDescriptor(actual,"0"),
                one=Object.getOwnPropertyDescriptor(actual,"1");
            return [__array(actual),state,log,Object.getPrototypeOf(actual)===Array.prototype,
                __bits(actual,"0"),__bits(actual,"1"),zero.value,one.value].join("|");
        })()
// case: global no-match returns null after the unconditional zero write
(function(){
            var log="",state=-0,receiver=Object();receiver.flags="g";
            Object.defineProperty(receiver,"lastIndex",{
                get:function(){log+="BAD-get;";return state},
                set:function(value){log+="set:"+__show(value)+";";state=value}
            });
            receiver.exec=function(){log+="exec;";return null};
            var actual=RegExp.prototype[Symbol.match].call(receiver,"x");
            return [actual,__show(state),log].join("|");
        })()
// case: native global regexps collect all matches and finish with the builtin lastIndex reset
(function(){
            var hit=/a/g;hit.lastIndex=3;
            var miss=/z/g;miss.lastIndex=2;
            var nonGlobal=/a/;nonGlobal.lastIndex=2;
            return [__array(RegExp.prototype[Symbol.match].call(hit,"baac")),hit.lastIndex,
                RegExp.prototype[Symbol.match].call(miss,"abc"),miss.lastIndex,
                __array(RegExp.prototype[Symbol.match].call(nonGlobal,"ba")),
                nonGlobal.lastIndex].join("|");
        })()
// EMPTY_ADVANCE_CASES
// case: empty global matches advance one UTF-16 unit or one Unicode code point including v
(function(){
            function run(flags,input,current){
                var state=99,calls=0,receiver=Object();receiver.flags=flags;
                Object.defineProperty(receiver,"lastIndex",{
                    get:function(){return state},set:function(value){state=value}
                });
                receiver.exec=function(){
                    calls++;
                    if(calls===1){state=current;return {0:""}}
                    return null;
                };
                var actual=RegExp.prototype[Symbol.match].call(receiver,input);
                return flags+":"+__show(current)+":"+__show(state)+":"+__array(actual);
            }
            var astral="\ud83d\ude00",lone="\ud800";
            return [run("g",astral,0),run("gu",astral,0),run("gv",astral,0),
                run("gu",astral,1),run("gu",astral,2),run("gu",lone,0),
                run("g","abc",-3),run("g","abc",1.9),
                run("g","abc",Infinity)].join("|");
        })()
// case: empty match reads and converts lastIndex only after defining the result element
(function(){
            var log="",state=8,calls=0,index=Object(),receiver=Object();receiver.flags="gu";
            Object.defineProperty(receiver,"lastIndex",{
                get:function(){log+="last-get;";return index},
                set:function(value){log+="last-set:"+__show(value)+";";state=value}
            });
            index.valueOf=function(){log+="index-value;";return 0};
            receiver.exec=function(){calls++;log+="exec:"+calls+";";return calls===1?{0:""}:null};
            var actual=RegExp.prototype[Symbol.match].call(receiver,"\ud83d\ude00");
            return [__array(actual),state,log].join("|");
        })()
// ABRUPT_CASES
// case: input flags and initial lastIndex abrupt completions stop at exact boundaries
(function(){
            function capture(receiver,input,sentinel){
                try{RegExp.prototype[Symbol.match].call(receiver,input);return "return"}
                catch(error){return (error===sentinel)+":"+receiver.log+":"+__show(receiver.state)}
            }
            var inputBoom=Object(),input=Object(),inputReceiver=Object();
            inputReceiver.log="";inputReceiver.state=3;
            input.toString=function(){inputReceiver.log+="input;";throw inputBoom};
            Object.defineProperty(inputReceiver,"flags",{get:function(){this.log+="BAD-flags;";return "g"}});

            var flagsBoom=Object(),flagsReceiver=Object();flagsReceiver.log="";flagsReceiver.state=3;
            Object.defineProperty(flagsReceiver,"flags",{get:function(){this.log+="flags;";throw flagsBoom}});

            var convertBoom=Object(),flags=Object(),convertReceiver=Object();
            convertReceiver.log="";convertReceiver.state=3;
            Object.defineProperty(convertReceiver,"flags",{get:function(){this.log+="flags;";return flags}});
            flags.toString=function(){convertReceiver.log+="flags-string;";throw convertBoom};

            var setBoom=Object(),setReceiver=Object();setReceiver.log="";setReceiver.state=3;setReceiver.flags="g";
            Object.defineProperty(setReceiver,"lastIndex",{
                get:function(){this.log+="BAD-get;";return this.state},
                set:function(value){this.log+="set:"+value+";";throw setBoom}
            });
            setReceiver.exec=function(){this.log+="BAD-exec;";return null};
            return [capture(inputReceiver,input,inputBoom),capture(flagsReceiver,"x",flagsBoom),
                capture(convertReceiver,"x",convertBoom),capture(setReceiver,"x",setBoom)].join("|");
        })()
// case: exec result zero and empty lastIndex abrupt completions preserve identity and effects
(function(){
            function capture(receiver,sentinel){
                try{RegExp.prototype[Symbol.match].call(receiver,"x");return "return"}
                catch(error){return (error===sentinel)+":"+receiver.log+":"+__show(receiver.state)}
            }
            function base(){
                var receiver=Object();receiver.flags="g";receiver.log="";receiver.state=7;
                Object.defineProperty(receiver,"lastIndex",{
                    get:function(){this.log+="get;";return this.state},
                    set:function(value){this.log+="set:"+value+";";this.state=value},
                    configurable:true
                });
                return receiver;
            }
            var execBoom=Object(),execReceiver=base();
            Object.defineProperty(execReceiver,"exec",{get:function(){this.log+="exec-get;";throw execBoom}});

            var zeroBoom=Object(),zeroReceiver=base(),zeroResult=Object();
            Object.defineProperty(zeroResult,"0",{get:function(){zeroReceiver.log+="zero-get;";throw zeroBoom}});
            zeroReceiver.exec=function(){this.log+="exec;";return zeroResult};

            var stringBoom=Object(),stringReceiver=base(),stringResult=Object(),zero=Object();
            zero.toString=function(){stringReceiver.log+="zero-string;";throw stringBoom};
            Object.defineProperty(stringResult,"0",{get:function(){stringReceiver.log+="zero-get;";return zero}});
            stringReceiver.exec=function(){this.log+="exec;";return stringResult};

            var getBoom=Object(),getReceiver=base(),getCalls=0;
            Object.defineProperty(getReceiver,"lastIndex",{
                get:function(){this.log+="get;";throw getBoom},
                set:function(value){this.log+="set:"+value+";";this.state=value}
            });
            getReceiver.exec=function(){getCalls++;this.log+="exec;";return getCalls===1?{0:""}:null};

            var convertBoom=Object(),convertReceiver=base(),convertCalls=0,index=Object();
            index.valueOf=function(){convertReceiver.log+="index-value;";throw convertBoom};
            Object.defineProperty(convertReceiver,"lastIndex",{
                get:function(){this.log+="get;";return index},
                set:function(value){this.log+="set:"+value+";";this.state=value}
            });
            convertReceiver.exec=function(){convertCalls++;this.log+="exec;";return convertCalls===1?{0:""}:null};

            var setBoom=Object(),setReceiver=base(),setCalls=0,setCount=0;
            Object.defineProperty(setReceiver,"lastIndex",{
                get:function(){this.log+="get;";return 0},
                set:function(value){this.log+="set:"+value+";";this.state=value;if(++setCount===2)throw setBoom}
            });
            setReceiver.exec=function(){setCalls++;this.log+="exec;";return setCalls===1?{0:""}:null};

            return [capture(execReceiver,execBoom),capture(zeroReceiver,zeroBoom),
                capture(stringReceiver,stringBoom),capture(getReceiver,getBoom),
                capture(convertReceiver,convertBoom),capture(setReceiver,setBoom)].join("|");
        })()
// RECURSION_CASES
// case: recursive String and RegExp match protocol calls are catchable and recover
(function(){
            var pattern=Object(),stringError="",regexp=Object(),regexpError="";
            pattern[Symbol.match]=function(value){return String.prototype.match.call(value,pattern)};
            try{"x".match(pattern)}catch(error){stringError=error.name+":"+error.message}
            regexp.flags="";
            regexp.exec=function(){return RegExp.prototype[Symbol.match].call(regexp,"x")};
            try{RegExp.prototype[Symbol.match].call(regexp,"x")}
            catch(error){regexpError=error.name+":"+error.message}
            return [stringError,regexpError,__array("abc".match("b")),
                RegExp.prototype[Symbol.match].call({flags:"",exec:function(){return null}},"x")].join("|");
        })()
