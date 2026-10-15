// prelude: regexp_match_all
// METADATA_CASES
// case: RegExp matchAll and iterator expose pinned metadata and key order
(function(){
            var fn=RegExp.prototype[Symbol.matchAll],
                iterator=/a/g[Symbol.matchAll]("a"),
                prototype=Object.getPrototypeOf(iterator),
                next=prototype.next,
                regexpKeys=Reflect.ownKeys(RegExp.prototype),
                selected=[],index=0,key;
            while(index<regexpKeys.length){
                key=regexpKeys[index++];
                if(key==="test"||key==="toString"||key===Symbol.match||
                    key===Symbol.matchAll||key===Symbol.search||key===Symbol.split)
                    selected[selected.length]=String(key);
            }
            return [
                selected.join(","),
                __bits(RegExp.prototype,Symbol.matchAll),fn.name,fn.length,
                Object.getOwnPropertyNames(fn).join(","),
                __bits(fn,"name"),__bits(fn,"length"),__isConstructor(fn),
                Object.prototype.hasOwnProperty.call(fn,"prototype"),
                Reflect.ownKeys(prototype).map(String).join(","),
                __bits(prototype,"next"),next.name,next.length,
                Object.getOwnPropertyNames(next).join(","),
                __bits(next,"name"),__bits(next,"length"),__isConstructor(next),
                __bits(prototype,Symbol.toStringTag),
                prototype[Symbol.toStringTag],
                iterator[Symbol.iterator]()===iterator,
                Object.prototype.toString.call(iterator)
            ].join("|");
        })()
// case: RegExp matchAll AutoInit identity is stable and independently replaceable
(function(){
            var first=RegExp.prototype[Symbol.matchAll],
                second=RegExp.prototype[Symbol.matchAll],
                stable=first===second,
                deleted=delete RegExp.prototype[Symbol.matchAll];
            RegExp.prototype[Symbol.matchAll]=17;
            return [stable,deleted,RegExp.prototype[Symbol.matchAll],
                __bits(RegExp.prototype,Symbol.matchAll),
                typeof first,first.name,first.length].join("|");
        })()
// CONSTRUCTION_CASES
// case: matchAll converts input then resolves species flags construction and lastIndex
(function(){
            var log="",receiver=Object(),holder=Object(),input=Object(),
                flags=Object(),length=Object(),matcher;
            input.toString=function(){log+="input-string;";return "subject"};
            Object.defineProperty(receiver,"constructor",{get:function(){
                log+="constructor-get;";return holder;
            }});
            Object.defineProperty(holder,Symbol.species,{get:function(){
                log+="species-get:"+(this===holder)+";";return Species;
            }});
            Object.defineProperty(receiver,"flags",{get:function(){
                log+="flags-get;";return flags;
            }});
            flags.toString=function(){log+="flags-string;";return "g"};
            Object.defineProperty(receiver,"lastIndex",{get:function(){
                log+="last-get;";return length;
            }});
            length.valueOf=function(){log+="last-number;";return 2.9};
            function Species(pattern,observedFlags){
                log+="construct:"+(new.target===Species)+":"+(pattern===receiver)+":"+
                    observedFlags+":"+arguments.length+";";
                matcher=Object();
                Object.defineProperty(matcher,"lastIndex",{
                    get:function(){return 0},
                    set:function(value){log+="matcher-last-set:"+value+";"}
                });
                matcher.exec=function(){return null};
                return matcher;
            }
            var iterator=RegExp.prototype[Symbol.matchAll].call(receiver,input);
            return log+"|"+Object.prototype.toString.call(iterator)+
                "|"+(iterator[Symbol.iterator]()===iterator);
        })()
// case: matchAll abrupt completions stop at exact construction boundaries
(function(){
            function primitive(){
                var log="",input=Object();
                input.toString=function(){log+="BAD-input;";return "x"};
                return __completion(function(){
                    return RegExp.prototype[Symbol.matchAll].call(1,input);
                })+":"+log;
            }
            function badSpecies(){
                var log="",receiver=Object(),holder=Object(),input=Object();
                input.toString=function(){log+="input;";return "x"};
                Object.defineProperty(receiver,"constructor",{get:function(){
                    log+="constructor;";return holder;
                }});
                Object.defineProperty(holder,Symbol.species,{get:function(){
                    log+="species;";return 1;
                }});
                Object.defineProperty(receiver,"flags",{get:function(){
                    log+="BAD-flags;";return "g";
                }});
                return __completion(function(){
                    return RegExp.prototype[Symbol.matchAll].call(receiver,input);
                })+":"+log;
            }
            function badFlags(){
                var log="",receiver=Object(),holder=Object(),flags=Object();
                receiver.constructor=holder;holder[Symbol.species]=function(){
                    log+="BAD-construct;";return Object();
                };
                Object.defineProperty(receiver,"flags",{get:function(){
                    log+="flags-get;";return flags;
                }});
                flags.toString=function(){log+="flags-string;";throw 73};
                return __completion(function(){
                    return RegExp.prototype[Symbol.matchAll].call(receiver,"x");
                })+":"+log;
            }
            function badConstruct(){
                var log="",receiver=Object(),holder=Object();
                receiver.constructor=holder;receiver.flags="g";
                holder[Symbol.species]=function(){log+="construct;";throw 74};
                Object.defineProperty(receiver,"lastIndex",{get:function(){
                    log+="BAD-last;";return 0;
                }});
                return __completion(function(){
                    return RegExp.prototype[Symbol.matchAll].call(receiver,"x");
                })+":"+log;
            }
            return [primitive(),badSpecies(),badFlags(),badConstruct()].join("|");
        })()
// case: nullish species uses the retained RegExp constructor and ignores global replacement
(function(){
            var intrinsicRegExp=RegExp,intrinsicIteratorPrototype=
                    Object.getPrototypeOf(/x/g[Symbol.matchAll]("")),
                method=RegExp.prototype[Symbol.matchAll],
                first=/a/g,second=/b/g,firstHolder=Object(),secondHolder=Object();
            firstHolder[Symbol.species]=null;
            secondHolder[Symbol.species]=undefined;
            first.constructor=firstHolder;second.constructor=secondHolder;
            globalThis.RegExp=function(){throw "global RegExp used"};
            var left=method.call(first,"a"),right=method.call(second,"b");
            return [
                __step(left),__step(left),__step(right),__step(right),
                Object.getPrototypeOf(left)===intrinsicIteratorPrototype,
                Object.getPrototypeOf(right)===intrinsicIteratorPrototype,
                Object.getPrototypeOf(first)===intrinsicRegExp.prototype
            ].join("|");
        })()
// case: source lastIndex is ToLength copied once without mutating the source
(function(){
            function run(initial){
                var receiver=Object(),holder=Object(),copied="missing",sets=0;
                receiver.constructor=holder;receiver.flags="g";receiver.lastIndex=initial;
                holder[Symbol.species]=function(){
                    var matcher=Object();
                    Object.defineProperty(matcher,"lastIndex",{set:function(value){
                        sets++;copied=__show(value);
                    }});
                    matcher.exec=function(){return null};
                    return matcher;
                };
                var iterator=RegExp.prototype[Symbol.matchAll].call(receiver,"x");
                return copied+":"+sets+":"+__show(receiver.lastIndex)+":"+
                    Object.prototype.toString.call(iterator);
            }
            var number=Object();number.valueOf=function(){return 3.8};
            return [run(-4),run(number),run(Infinity)].join("|");
        })()
// ITERATION_CASES
// case: non-global iterators finish after one match while global iterators run to null
(function(){
            function run(flags){
                var receiver=Object(),holder=Object(),calls=0;
                receiver.constructor=holder;receiver.flags=flags;receiver.lastIndex=0;
                holder[Symbol.species]=function(){
                    return {lastIndex:0,exec:function(){
                        calls++;
                        if(calls<=2)return {0:"m"+calls,index:calls};
                        return null;
                    }};
                };
                var iterator=RegExp.prototype[Symbol.matchAll].call(receiver,"subject");
                return [__step(iterator),__step(iterator),__step(iterator),
                    __step(iterator),calls].join(",");
            }
            return run("")+"|"+run("g");
        })()
// case: real RegExp iterators copy lastIndex and preserve the source regexp
(function(){
            var global=/a/g,plain=/a/;
            global.lastIndex=1;plain.lastIndex=2;
            var globalIterator=RegExp.prototype[Symbol.matchAll].call(global,"baac"),
                plainIterator=RegExp.prototype[Symbol.matchAll].call(plain,"baac");
            return [
                __step(globalIterator),__step(globalIterator),__step(globalIterator),
                global.lastIndex,
                __step(plainIterator),__step(plainIterator),plain.lastIndex
            ].join("|");
        })()
// case: empty global matches advance by code unit or Unicode code point from matcher lastIndex
(function(){
            function run(flags){
                var receiver=Object(),holder=Object(),state=0,log="",calls=0;
                receiver.constructor=holder;receiver.flags=flags;receiver.lastIndex=0;
                holder[Symbol.species]=function(){
                    var matcher=Object();
                    Object.defineProperty(matcher,"lastIndex",{
                        get:function(){log+="get:"+state+";";return state},
                        set:function(value){log+="set:"+value+";";state=value}
                    });
                    matcher.exec=function(){
                        log+="exec:"+state+";";calls++;
                        return calls===1?{0:"",index:state}:null;
                    };
                    return matcher;
                };
                var iterator=RegExp.prototype[Symbol.matchAll].call(
                    receiver,String.fromCharCode(0xd83d,0xde00,0x58));
                return [__step(iterator),__step(iterator),__step(iterator),log].join(",");
            }
            return [run(""),run("g"),run("gu"),run("gv")].join("|");
        })()
// case: iterator next result objects preserve match identity and stable done state
(function(){
            var receiver=Object(),holder=Object(),match=Object(),calls=0;
            match[0]="x";match.index=4;
            receiver.constructor=holder;receiver.flags="g";receiver.lastIndex=0;
            holder[Symbol.species]=function(){
                return {lastIndex:0,exec:function(){calls++;return calls===1?match:null}};
            };
            var iterator=RegExp.prototype[Symbol.matchAll].call(receiver,"subject"),
                first=iterator.next(),second=iterator.next(),third=iterator.next();
            return [
                first.value===match,first.done,
                second.value,second.done,
                third.value,third.done,calls,
                Object.getPrototypeOf(first)===Object.prototype,
                Object.getPrototypeOf(second)===Object.prototype
            ].join("|");
        })()
// ERROR_CASES
// case: iterator next rejects incompatible receivers without running user code
(function(){
            var iterator=/a/g[Symbol.matchAll]("a"),
                next=Object.getPrototypeOf(iterator).next;
            return [
                __completion(function(){return next.call(undefined)}),
                __completion(function(){return next.call(null)}),
                __completion(function(){return next.call(1)}),
                __completion(function(){return next.call(Object())}),
                __step(iterator),__step(iterator)
            ].join("|");
        })()
// case: exec throws and primitive results leave the iterator retryable
(function(){
            var receiver=Object(),holder=Object(),calls=0;
            receiver.constructor=holder;receiver.flags="g";receiver.lastIndex=0;
            holder[Symbol.species]=function(){
                return {lastIndex:0,exec:function(){
                    calls++;
                    if(calls===1)throw 71;
                    if(calls===2)return 1;
                    return null;
                }};
            };
            var iterator=RegExp.prototype[Symbol.matchAll].call(receiver,"x");
            return [
                __completion(function(){return iterator.next()}),
                __completion(function(){return iterator.next()}),
                __step(iterator),__step(iterator),calls
            ].join("|");
        })()
// case: match zero and empty-match lastIndex errors leave the iterator retryable
(function(){
            function zeroGetter(){
                var receiver=Object(),holder=Object(),calls=0,match=Object();
                receiver.constructor=holder;receiver.flags="g";receiver.lastIndex=0;
                Object.defineProperty(match,"0",{get:function(){throw 72}});
                holder[Symbol.species]=function(){
                    return {lastIndex:0,exec:function(){
                        calls++;return calls===1?match:null;
                    }};
                };
                var iterator=RegExp.prototype[Symbol.matchAll].call(receiver,"x");
                return __completion(function(){return iterator.next()})+":"+
                    __step(iterator)+":"+__step(iterator)+":"+calls;
            }
            function lastIndexGetter(){
                var receiver=Object(),holder=Object(),calls=0,state=0;
                receiver.constructor=holder;receiver.flags="g";receiver.lastIndex=0;
                holder[Symbol.species]=function(){
                    var matcher=Object();
                    Object.defineProperty(matcher,"lastIndex",{
                        get:function(){throw 73},
                        set:function(value){state=value}
                    });
                    matcher.exec=function(){
                        calls++;return calls===1?{0:"",index:state}:null;
                    };
                    return matcher;
                };
                var iterator=RegExp.prototype[Symbol.matchAll].call(receiver,"x");
                return __completion(function(){return iterator.next()})+":"+
                    __step(iterator)+":"+__step(iterator)+":"+calls;
            }
            return zeroGetter()+"|"+lastIndexGetter();
        })()
