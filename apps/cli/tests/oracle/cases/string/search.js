// prelude: regexp_completion
// METADATA_CASES
// case: search methods expose pinned metadata descriptors and key order
(function(){
            var stringFn=String.prototype.search,
                regexpFn=RegExp.prototype[Symbol.search],
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
                if(key==="test"||key==="toString"||key==="constructor"||key===Symbol.search)
                    regexpOrder[regexpOrder.length]=String(key);
                index++;
            }
            return [
                stringOrder.join(","),regexpOrder.join(","),
                __bits(String.prototype,"search"),stringFn.name,stringFn.length,
                Object.getOwnPropertyNames(stringFn).join(","),
                __bits(stringFn,"name"),__bits(stringFn,"length"),
                __isConstructor(stringFn),
                Object.prototype.hasOwnProperty.call(stringFn,"prototype"),
                __bits(RegExp.prototype,Symbol.search),regexpFn.name,regexpFn.length,
                Object.getOwnPropertyNames(regexpFn).join(","),
                __bits(regexpFn,"name"),__bits(regexpFn,"length"),
                __isConstructor(regexpFn),
                Object.prototype.hasOwnProperty.call(regexpFn,"prototype")
            ].join("|");
        })()
// STRING_DISPATCH_CASES
// case: String search delegates before conversion and returns the custom result unchanged
(function(){
            var log="",receiver=Object(),regexp=Object(),result=Object();
            receiver.toString=function(){log+="BAD-receiver-string;";throw "converted"};
            Object.defineProperty(regexp,Symbol.search,{get:function(){
                log+="get;";
                return function(value){
                    log+="call:"+(this===regexp)+":"+(value===receiver)+":"+
                        arguments.length+";";
                    return result;
                };
            }});
            var actual=String.prototype.search.call(receiver,regexp);
            return (actual===result)+"|"+log;
        })()
// case: String search rejection order noncallable hooks and primitive-pattern bypass
(function(){
            var log="",poison=Object(),receiver=Object(),noncallable=Object(),hits=0;
            Object.defineProperty(poison,Symbol.search,{get:function(){log+="BAD-nullish;";return null}});
            var nullish=__completion(function(){return String.prototype.search.call(null,poison)});
            receiver.toString=function(){log+="BAD-noncallable-string;";return "a"};
            Object.defineProperty(noncallable,Symbol.search,{get:function(){log+="hook-get;";return 17}});
            var invalid=__completion(function(){return String.prototype.search.call(receiver,noncallable)});
            Object.defineProperty(Number.prototype,Symbol.search,{configurable:true,get:function(){
                hits++;throw "primitive hook observed";
            }});
            var fallbackReceiver=Object();
            fallbackReceiver.toString=function(){log+="fallback-string;";return "a7"};
            var fallback=String.prototype.search.call(fallbackReceiver,7);
            return [nullish,invalid,fallback,hits,log].join("|");
        })()
// STRING_FALLBACK_CASES
// case: String search uses the intrinsic constructor and pinned fallback conversion order
(function(){
            var log="",intrinsic=RegExp,receiver=Object(),pattern=Object(),
                source=Object(),flags=Object();
            Object.defineProperty(pattern,Symbol.search,{get:function(){log+="pattern-search;";return null}});
            Object.defineProperty(pattern,Symbol.match,{get:function(){log+="pattern-match;";return true}});
            Object.defineProperty(pattern,"source",{get:function(){log+="source-get;";return source}});
            Object.defineProperty(pattern,"flags",{get:function(){log+="flags-get;";return flags}});
            Object.defineProperty(pattern,"constructor",{get:function(){log+="BAD-constructor;";return intrinsic}});
            receiver.toString=function(){log+="receiver-string;";return "subject"};
            source.toString=function(){log+="source-string;";return "sub"};
            flags.toString=function(){log+="flags-string;";return ""};
            Object.defineProperty(intrinsic.prototype,Symbol.search,{configurable:true,get:function(){
                log+="new-search;";
                return function(value){
                    log+="new-call:"+(Object.getPrototypeOf(this)===intrinsic.prototype)+":"+
                        (value==="subject")+":"+__show(this.lastIndex)+";";
                    return 73;
                };
            }});
            globalThis.RegExp=function(){log+="BAD-global;";throw "global constructor used"};
            var result=String.prototype.search.call(receiver,pattern);
            return result+"|"+log;
        })()
// LAST_INDEX_CASES
// case: RegExp Symbol.search converts then restores negative zero before reading raw index
(function(){
            var log="",state=-0,index=Object(),input=Object(),receiver=Object(),result=Object();
            input.toString=function(){log+="input-string;";return "abc"};
            Object.defineProperty(receiver,"lastIndex",{
                get:function(){log+="get:"+__show(state)+";";return state},
                set:function(value){log+="set:"+__show(value)+";";state=value}
            });
            Object.defineProperty(receiver,"exec",{get:function(){
                log+="exec-get;";
                return function(value){log+="exec-call:"+(this===receiver)+":"+value+";";return result};
            }});
            Object.defineProperty(result,"index",{get:function(){log+="index-get;";return index}});
            var actual=RegExp.prototype[Symbol.search].call(receiver,input);
            return [(actual===index),__show(state),log].join("|");
        })()
// case: RegExp Symbol.search uses SameValue for positive zero and NaN
(function(){
            function run(initial,writeNaN){
                var log="",state=initial,receiver=Object();
                Object.defineProperty(receiver,"lastIndex",{
                    get:function(){log+="get:"+__show(state)+";";return state},
                    set:function(value){log+="set:"+__show(value)+";";state=value}
                });
                receiver.exec=function(){
                    log+="exec;";
                    if(writeNaN)this.lastIndex=0/0;
                    return null;
                };
                var result=RegExp.prototype[Symbol.search].call(receiver,"x");
                return result+":"+__show(state)+":"+log;
            }
            return run(0,false)+"|"+run(0/0,true);
        })()
// ABRUPT_CASES
// case: RegExp Symbol.search stops at initial lastIndex and exec abrupt boundaries
(function(){
            function make(){
                var receiver=Object();receiver.state=5;receiver.log="";
                Object.defineProperty(receiver,"lastIndex",{
                    get:function(){this.log+="get;";return this.state},
                    set:function(value){this.log+="set:"+value+";";this.state=value}
                });
                return receiver;
            }
            function capture(receiver,sentinel){
                try{RegExp.prototype[Symbol.search].call(receiver,"x");return "return"}
                catch(error){
                    var kind=error===sentinel?"same":error.name+":"+error.message;
                    return kind+":"+receiver.state+":"+receiver.log;
                }
            }
            var initialGet=Object(),initialGetBoom=Object();
            initialGet.state=5;initialGet.log="";
            Object.defineProperty(initialGet,"lastIndex",{
                get:function(){this.log+="get;";throw initialGetBoom},
                set:function(){this.log+="BAD-set;"}
            });
            initialGet.exec=function(){this.log+="BAD-exec;";return null};

            var initialSet=Object(),initialSetBoom=Object();
            initialSet.state=5;initialSet.log="";
            Object.defineProperty(initialSet,"lastIndex",{
                get:function(){this.log+="get;";return this.state},
                set:function(value){this.log+="set:"+value+";";throw initialSetBoom}
            });
            initialSet.exec=function(){this.log+="BAD-exec;";return null};

            var first=make(),firstBoom=Object();
            Object.defineProperty(first,"exec",{get:function(){this.log+="exec-get;";throw firstBoom}});
            var second=make(),secondBoom=Object();
            second.exec=function(){this.log+="exec-call;";this.state=9;throw secondBoom};
            var third=make();
            third.exec=function(){this.log+="exec-call;";this.state=9;return 1};
            return [capture(initialGet,initialGetBoom),capture(initialSet,initialSetBoom),
                capture(first,firstBoom),capture(second,secondBoom),capture(third,null)].join("|");
        })()
// case: RegExp Symbol.search post-exec failures bracket restoration and index access
(function(){
            function capture(receiver,sentinel){
                try{RegExp.prototype[Symbol.search].call(receiver,"x");return "return"}
                catch(error){
                    return (error===sentinel)+":"+receiver.state+":"+receiver.log;
                }
            }
            var current=Object(),currentBoom=Object(),currentGets=0;
            current.state=5;current.log="";
            Object.defineProperty(current,"lastIndex",{
                get:function(){this.log+="get:"+(++currentGets)+";";if(currentGets===2)throw currentBoom;return this.state},
                set:function(value){this.log+="set:"+value+";";this.state=value}
            });
            current.exec=function(){this.log+="exec;";return null};

            var restore=Object(),restoreBoom=Object(),restoreSets=0,restoreResult=Object();
            restore.state=5;restore.log="";
            Object.defineProperty(restore,"lastIndex",{
                get:function(){this.log+="get;";return this.state},
                set:function(value){this.log+="set:"+value+";";if(++restoreSets===2)throw restoreBoom;this.state=value}
            });
            Object.defineProperty(restoreResult,"index",{get:function(){restore.log+="BAD-index;";return 1}});
            restore.exec=function(){this.log+="exec;";return restoreResult};

            var index=Object(),indexBoom=Object(),indexResult=Object();
            index.state=5;index.log="";
            Object.defineProperty(index,"lastIndex",{
                get:function(){this.log+="get;";return this.state},
                set:function(value){this.log+="set:"+value+";";this.state=value}
            });
            Object.defineProperty(indexResult,"index",{get:function(){index.log+="index;";throw indexBoom}});
            index.exec=function(){this.log+="exec;";this.state=8;return indexResult};

            return [capture(current,currentBoom),capture(restore,restoreBoom),
                capture(index,indexBoom)].join("|");
        })()
// ABSTRACT_EXEC_CASES
// case: RegExp Symbol.search honors builtin fallback branding null and raw index results
(function(){
            var branded=/b/g;branded.lastIndex=7;branded.exec=null;
            var brandedResult=RegExp.prototype[Symbol.search].call(branded,"abc");

            var ordinary=Object();ordinary.exec=null;
            var ordinaryCompletion=__completion(function(){
                return RegExp.prototype[Symbol.search].call(ordinary,"abc");
            });

            var miss=Object();miss.exec=function(){return null};
            var missResult=RegExp.prototype[Symbol.search].call(miss,"abc");

            var raw=Object();raw.exec=function(){var result=Object();result.index="raw";return result};
            var rawResult=RegExp.prototype[Symbol.search].call(raw,"abc");

            return [brandedResult,branded.lastIndex,ordinaryCompletion,
                Object.prototype.hasOwnProperty.call(ordinary,"lastIndex"),__show(ordinary.lastIndex),
                missResult,Object.prototype.hasOwnProperty.call(miss,"lastIndex"),__show(miss.lastIndex),
                rawResult,Object.prototype.hasOwnProperty.call(raw,"lastIndex"),__show(raw.lastIndex)
            ].join("|");
        })()
