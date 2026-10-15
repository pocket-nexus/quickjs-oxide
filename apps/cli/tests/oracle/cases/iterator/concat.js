// case: intrinsic graph / static metadata hidden prototype descriptors tag and exact brands
(function(){
            try {
                function bit(value){return value?"1":"0"}
                function bits(descriptor){
                    return bit(descriptor.writable)+bit(descriptor.enumerable)+
                        bit(descriptor.configurable);
                }
                function keyName(key){
                    if(key===Symbol.toStringTag)return "@@toStringTag";
                    return String(key);
                }
                function keys(object){
                    return Reflect.ownKeys(object).map(keyName).join(",");
                }
                function errorName(thunk){
                    try{thunk();return "none"}
                    catch(error){
                        return error&&error.name?error.name:String(error);
                    }
                }
                var empty=Iterator.concat();
                var prototype=Object.getPrototypeOf(empty);
                var helperPrototype=Object.getPrototypeOf(
                    [1].values().map(function(value){return value}));
                var wrapPrototype=Object.getPrototypeOf(
                    Iterator.from({next:function(){return {done:true}}}));
                var nextDescriptor=
                    Object.getOwnPropertyDescriptor(prototype,"next");
                var returnDescriptor=
                    Object.getOwnPropertyDescriptor(prototype,"return");
                var tagDescriptor=
                    Object.getOwnPropertyDescriptor(prototype,Symbol.toStringTag);
                var concatDescriptor=
                    Object.getOwnPropertyDescriptor(Iterator,"concat");
                return "static="+keys(Iterator)+
                    "|concat="+Iterator.concat.name+":"+Iterator.concat.length+":"+
                        bits(concatDescriptor)+":"+
                        errorName(function(){new Iterator.concat()})+
                    "|proto="+
                        (Object.getPrototypeOf(prototype)===Iterator.prototype)+":"+
                        (prototype!==Iterator.prototype)+":"+
                        (prototype!==helperPrototype)+":"+
                        (prototype!==wrapPrototype)+":"+keys(prototype)+
                    "|methods="+
                        prototype.next.name+":"+prototype.next.length+":"+
                            bits(nextDescriptor)+","+
                        prototype.return.name+":"+prototype.return.length+":"+
                            bits(returnDescriptor)+
                    "|tag="+tagDescriptor.value+":"+bits(tagDescriptor)+":"+
                        Object.prototype.toString.call(empty)+
                    "|brand="+
                        errorName(function(){prototype.next.call({})})+":"+
                        errorName(function(){prototype.return.call({})});
            } catch(error) {
                return "case-error:"+
                    (error&&error.name?error.name:String(error));
            }
        })()
// case: lazy sequencing / iterator getters are eager while opening and cached next calls are lazy
(function(){
            try {
                var log=[];
                function make(label,values){
                    var index=0,iterator;
                    var iterable={};
                    Object.defineProperty(iterable,Symbol.iterator,{
                        configurable:true,
                        get:function(){
                            log.push("getiter-"+label);
                            return function(){
                                log.push("open-"+label+":"+
                                    String(this===iterable)+":"+
                                    arguments.length);
                                iterator={};
                                Object.defineProperty(iterator,"next",{
                                    configurable:true,
                                    get:function(){
                                        log.push("getnext-"+label);
                                        return function(){
                                            log.push("next-"+label+":"+
                                                String(this===iterator)+":"+
                                                arguments.length);
                                            if(index===values.length){
                                                return {
                                                    done:true,
                                                    get value(){
                                                        log.push("badvalue-"+label);
                                                        return 0;
                                                    }
                                                };
                                            }
                                            return {
                                                done:false,
                                                value:values[index++]
                                            };
                                        };
                                    }
                                });
                                return iterator;
                            };
                        }
                    });
                    return iterable;
                }
                var sequence=Iterator.concat(
                    make("A",[10]),make("B",[20]));
                var eager=log.join(",");
                log.push("made");
                var first=sequence.next();
                var second=sequence.next();
                var done=sequence.next();
                return "eager="+eager+
                    "|values="+first.value+":"+first.done+","+
                        second.value+":"+second.done+","+
                        String(done.value)+":"+done.done+
                    "|log="+log.join(",");
            } catch(error) {
                return "case-error:"+
                    (error&&error.name?error.name:String(error));
            }
        })()
// case: captured methods / static this capture cache sentinel and primitive-open retry boundaries
(function(){
            try {
                function errorName(thunk){
                    try{thunk();return "none"}
                    catch(error){return error&&error.name?error.name:String(error)}
                }
                var thisReads=0,poisonThis={};
                Object.defineProperty(poisonThis,Symbol.iterator,{
                    get:function(){thisReads++;throw "this-read"}
                });
                var oldCalls=0,newCalls=0;
                var capturedIterable={
                    [Symbol.iterator]:function(){
                        oldCalls++;
                        return {next:function(){return {done:false,value:21}}};
                    }
                };
                var captured=
                    Iterator.concat.call(poisonThis,capturedIterable);
                capturedIterable[Symbol.iterator]=
                    function(){newCalls++;throw "new-open"};
                var capturedStep=captured.next();
                var noncallableGets=0,noncallableIterator={};
                Object.defineProperty(noncallableIterator,"next",{
                    get:function(){noncallableGets++;return 1}
                });
                var noncallable=Iterator.concat({
                    [Symbol.iterator]:function(){return noncallableIterator}
                });
                var noncallableFirst=
                    errorName(function(){noncallable.next()});
                var noncallableSecond=
                    errorName(function(){noncallable.next()});
                var undefinedGets=0,undefinedIterator={};
                Object.defineProperty(undefinedIterator,"next",{
                    get:function(){
                        undefinedGets++;
                        if(undefinedGets===1)return undefined;
                        return function(){return {done:false,value:22}};
                    }
                });
                var undefinedSequence=Iterator.concat({
                    [Symbol.iterator]:function(){return undefinedIterator}
                });
                var undefinedFirst=
                    errorName(function(){undefinedSequence.next()});
                var undefinedStep=undefinedSequence.next();
                var openCalls=0;
                var primitiveOpen=function(){
                    openCalls++;
                    if(openCalls===1)return 1;
                    return {next:function(){return {done:false,value:23}}};
                };
                var primitiveIterable={[Symbol.iterator]:primitiveOpen};
                var primitiveSequence=Iterator.concat(primitiveIterable);
                primitiveIterable[Symbol.iterator]=
                    function(){throw "new-primitive-open"};
                var primitiveFirst=
                    errorName(function(){primitiveSequence.next()});
                var primitiveStep=primitiveSequence.next();
                return "this="+thisReads+
                    "|captured="+capturedStep.value+":"+
                        capturedStep.done+":"+oldCalls+":"+newCalls+
                    "|noncallable="+noncallableFirst+":"+
                        noncallableSecond+":"+noncallableGets+
                    "|undefined="+undefinedFirst+":"+
                        undefinedStep.value+":"+undefinedStep.done+":"+
                        undefinedGets+
                    "|primitive="+primitiveFirst+":"+
                        primitiveStep.value+":"+primitiveStep.done+":"+
                        openCalls;
            } catch(error) {
                return "case-error:"+
                    (error&&error.name?error.name:String(error));
            }
        })()
// case: abrupt retry / open next getter call done and value failures retry without closing
(function(){
            try {
                var closeCount=0,parts=[];
                function close(){
                    closeCount++;
                    return {};
                }
                function attempt(sequence){
                    try {
                        var result=sequence.next();
                        return "return:"+String(result.value)+":"+result.done;
                    } catch(error) {
                        return "throw:"+String(error);
                    }
                }
                function run(label,iterable,log){
                    var sequence=Iterator.concat(iterable);
                    parts.push(label+"="+attempt(sequence)+","+
                        attempt(sequence)+":"+log.join(","));
                }

                var openLog=[],openCount=0;
                run("open",{
                    [Symbol.iterator]:function(){
                        openLog.push("open"+(++openCount));
                        if(openCount===1)throw "open-error";
                        return {
                            next:function(){
                                openLog.push("next");
                                return {done:false,value:11};
                            },
                            return:close
                        };
                    }
                },openLog);

                var getLog=[],getCount=0,getIterator={return:close};
                Object.defineProperty(getIterator,"next",{
                    get:function(){
                        getLog.push("get"+(++getCount));
                        if(getCount===1)throw "get-error";
                        return function(){
                            getLog.push("next");
                            return {done:false,value:12};
                        };
                    }
                });
                run("get",{
                    [Symbol.iterator]:function(){return getIterator}
                },getLog);

                var callLog=[],callCount=0,callIterator={return:close};
                Object.defineProperty(callIterator,"next",{
                    get:function(){
                        callLog.push("get");
                        return function(){
                            callLog.push("next"+(++callCount));
                            if(callCount===1)throw "call-error";
                            return {done:false,value:13};
                        };
                    }
                });
                run("call",{
                    [Symbol.iterator]:function(){return callIterator}
                },callLog);

                var doneLog=[],doneCount=0;
                var doneIterator={
                    next:function(){
                        var current=++doneCount;
                        doneLog.push("next"+current);
                        return {
                            get done(){
                                doneLog.push("done"+current);
                                if(current===1)throw "done-error";
                                return false;
                            },
                            value:14
                        };
                    },
                    return:close
                };
                run("done",{
                    [Symbol.iterator]:function(){return doneIterator}
                },doneLog);

                var valueLog=[],valueCount=0;
                var valueIterator={
                    next:function(){
                        var current=++valueCount;
                        valueLog.push("next"+current);
                        return {
                            get done(){
                                valueLog.push("done"+current);
                                return false;
                            },
                            get value(){
                                valueLog.push("value"+current);
                                if(current===1)throw "value-error";
                                return 15;
                            }
                        };
                    },
                    return:close
                };
                run("value",{
                    [Symbol.iterator]:function(){return valueIterator}
                },valueLog);
                return parts.join("|")+"|closes="+closeCount;
            } catch(error) {
                return "case-error:"+
                    (error&&error.name?error.name:String(error));
            }
        })()
// case: return / before-start preserve drain validation and passthrough semantics
(function(){
            try {
                function resultText(result){
                    return String(result.value)+":"+result.done;
                }
                function completion(thunk){
                    try{return "return:"+String(thunk())}
                    catch(error){
                        return "throw:"+
                            (error&&error.name?error.name:String(error));
                    }
                }

                var beforeOpens=0;
                var before=Iterator.concat({
                    [Symbol.iterator]:function(){
                        beforeOpens++;
                        return {
                            next:function(){
                                return {done:false,value:1};
                            }
                        };
                    }
                });
                var beforeReturn=before.return("ignored");
                var beforeNext=before.next();

                var getterCount=0,getterIndex=0;
                var getterReceiver=false,getterArgc=-1,marker={};
                var getterIterator={
                    next:function(){
                        return {done:false,value:++getterIndex};
                    }
                };
                Object.defineProperty(getterIterator,"return",{
                    get:function(){
                        getterCount++;
                        if(getterCount===1)throw "return-get";
                        return function(){
                            getterReceiver=this===getterIterator;
                            getterArgc=arguments.length;
                            return marker;
                        };
                    }
                });
                var preserve=Iterator.concat({
                    [Symbol.iterator]:function(){return getterIterator}
                });
                var preserveFirst=preserve.next();
                var preserveError=completion(function(){
                    return preserve.return();
                });
                var preserveSecond=preserve.next();
                var preserveResult=preserve.return("ignored");
                var preserveDone=preserve.next();

                var drainSecondOpens=0;
                var drainIterator={
                    next:function(){return {done:false,value:7}},
                    return:function(){throw "return-call"}
                };
                var drain=Iterator.concat(
                    {
                        [Symbol.iterator]:function(){
                            return drainIterator;
                        }
                    },
                    {
                        [Symbol.iterator]:function(){
                            drainSecondOpens++;
                            return {
                                next:function(){return {done:true}}
                            };
                        }
                    }
                );
                drain.next();
                var drainError=completion(function(){
                    return drain.return();
                });
                var drainNext=drain.next();
                var drainAgain=drain.return();

                function invalid(value,present){
                    var iterator={
                        next:function(){
                            return {done:false,value:8};
                        }
                    };
                    if(present)iterator.return=value;
                    var sequence=Iterator.concat({
                        [Symbol.iterator]:function(){return iterator}
                    });
                    sequence.next();
                    var error=completion(function(){
                        return sequence.return();
                    });
                    return error+":"+resultText(sequence.next());
                }

                var primitiveIterator={
                    next:function(){return {done:false,value:9}},
                    return:function(){return 42}
                };
                var primitive=Iterator.concat({
                    [Symbol.iterator]:function(){
                        return primitiveIterator;
                    }
                });
                primitive.next();
                var primitiveResult=primitive.return("ignored");

                return "before="+String(beforeReturn)+":"+beforeOpens+":"+
                        resultText(beforeNext)+
                    "|getter="+resultText(preserveFirst)+":"+
                        preserveError+":"+resultText(preserveSecond)+":"+
                        (preserveResult===marker)+":"+getterReceiver+":"+
                        getterArgc+":"+resultText(preserveDone)+
                    "|drain="+drainError+":"+resultText(drainNext)+":"+
                        String(drainAgain)+":"+drainSecondOpens+
                    "|missing="+invalid(undefined,false)+
                    "|noncallable="+invalid(1,true)+
                    "|primitive="+typeof primitiveResult+":"+
                        primitiveResult+":"+resultText(primitive.next());
            } catch(error) {
                return "case-error:"+
                    (error&&error.name?error.name:String(error));
            }
        })()
// case: running guard / all next and return reentry directions throw while outer work survives
(function(){
            try {
                var log=[];
                function catchName(label,thunk){
                    try {
                        thunk();
                        log.push(label+":none");
                    } catch(error) {
                        log.push(label+":"+
                            (error&&error.name?error.name:String(error)));
                    }
                }
                function result(value){
                    return value.value+":"+value.done;
                }

                var nextNext;
                var nextNextIterable={
                    [Symbol.iterator]:function(){
                        catchName("nn",function(){nextNext.next()});
                        return {
                            next:function(){
                                return {done:false,value:1};
                            }
                        };
                    }
                };
                nextNext=Iterator.concat(nextNextIterable);
                var nextNextResult=nextNext.next();

                var nextReturn;
                var nextReturnIterator={
                    next:function(){
                        catchName("nr",function(){
                            nextReturn.return();
                        });
                        return {done:false,value:2};
                    }
                };
                nextReturn=Iterator.concat({
                    [Symbol.iterator]:function(){
                        return nextReturnIterator;
                    }
                });
                var nextReturnResult=nextReturn.next();

                var returnNext;
                var returnNextIterator={
                    next:function(){return {done:false,value:3}}
                };
                Object.defineProperty(returnNextIterator,"return",{
                    get:function(){
                        catchName("rn",function(){
                            returnNext.next();
                        });
                        return function(){return 30};
                    }
                });
                returnNext=Iterator.concat({
                    [Symbol.iterator]:function(){
                        return returnNextIterator;
                    }
                });
                returnNext.next();
                var returnNextResult=returnNext.return();

                var returnReturn;
                var returnReturnIterator={
                    next:function(){return {done:false,value:4}},
                    return:function(){
                        catchName("rr",function(){
                            returnReturn.return();
                        });
                        return 40;
                    }
                };
                returnReturn=Iterator.concat({
                    [Symbol.iterator]:function(){
                        return returnReturnIterator;
                    }
                });
                returnReturn.next();
                var returnReturnResult=returnReturn.return();

                return "results="+result(nextNextResult)+","+
                        result(nextReturnResult)+","+
                        returnNextResult+","+returnReturnResult+
                    "|log="+log.join(",");
            } catch(error) {
                return "case-error:"+
                    (error&&error.name?error.name:String(error));
            }
        })()
// case: raw IteratorNext stack / the direct native inner step is absent from the visible backtrace
(function(){
            try {
                Iterator.concat({
                    [Symbol.iterator]:function(){
                        return {
                            next:Object.getPrototypeOf(
                                [][Symbol.iterator]()).next
                        };
                    }
                }).next();
                return "no-error";
            } catch(error) {
                var stack=String(error.stack);
                var needle="    at next (native)";
                var count=0,index=0;
                while((index=stack.indexOf(needle,index))!==-1){
                    count++;
                    index+=needle.length;
                }
                return "TypeError="+
                    String(error&&error.name==="TypeError")+
                    "|native-next="+count;
            }
        })()
