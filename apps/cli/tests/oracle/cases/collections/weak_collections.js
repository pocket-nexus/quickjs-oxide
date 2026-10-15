function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+__bit(descriptor.configurable);
}
function __isConstructor(value){
    try{Reflect.construct(function(){},[],value);return true}catch(_error){return false}
}
function __completion(thunk){
    try{return "return:"+String(thunk())}
    catch(error){
        if(error!==null&&typeof error==="object")return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
function __keys(object){
    return Reflect.ownKeys(object).map(function(key){return String(key)}).join(",");
}
function __metadata(owner,key){
    var fn=owner[key];
    return String(key)+":"+fn.name+":"+fn.length+":"+__isConstructor(fn)+":"+
        __bits(owner,key)+":"+__bits(fn,"name")+":"+__bits(fn,"length")+":"+__keys(fn);
}
// case: graph / globals constructors prototypes descriptors and own-key order
(function(){
            function graph(name,constructor,prototype,methodNames){
                var methods=[],index;
                for(index=0;index<methodNames.length;index++)
                    methods[index]=__metadata(prototype,methodNames[index]);
                return [
                    name+"-global="+__bits(globalThis,name)+":"+
                        (Object.getOwnPropertyDescriptor(globalThis,name).value===constructor),
                    "constructor="+constructor.name+":"+constructor.length+":"+
                        __isConstructor(constructor)+":"+__keys(constructor),
                    "links="+(Object.getPrototypeOf(constructor)===Function.prototype)+":"+
                        (Object.getPrototypeOf(prototype)===Object.prototype)+":"+
                        (prototype.constructor===constructor),
                    "descriptors="+__bits(constructor,"prototype")+":"+
                        __bits(prototype,"constructor"),
                    "prototype-keys="+__keys(prototype),
                    "methods="+methods.join(";"),
                    "tag="+__bits(prototype,Symbol.toStringTag)+":"+
                        prototype[Symbol.toStringTag]+":"+Object.prototype.toString.call(prototype)+":"+
                        Object.prototype.toString.call(Reflect.construct(constructor,[]))
                ].join("|");
            }
            return graph("WeakMap",WeakMap,WeakMap.prototype,
                ["set","get","getOrInsert","getOrInsertComputed","has","delete"])+"||"+
                graph("WeakSet",WeakSet,WeakSet.prototype,["add","has","delete"]);
        })()
// case: core / object keys preserve identity update delete and fluent receivers
(function(){
            var first=Object(),second=Object(),other=Object();
            var map=new WeakMap(),set=new WeakSet();
            var mapChain=map.set(first,1)===map;
            map.set(first,2).set(second,3);
            var setChain=set.add(first)===set;
            set.add(second);
            return [
                mapChain,map.get(first),map.get(second),map.get(other),
                map.has(first),map.has(other),map.delete(first),map.delete(first),
                map.has(first),map.get(second),
                setChain,set.has(first),set.has(second),set.has(other),
                set.delete(first),set.delete(first),set.has(first),set.has(second),
                __completion(function(){return map.set(1,1)}),
                __completion(function(){return set.add("x")}),
                map.get(1),map.has(1),map.delete(1),set.has("x"),set.delete("x")
            ].join("|");
        })()
// case: stale-key / expired weak keys do not poison later ordinary property mutations
(function(){
            var map=new WeakMap(),set=new WeakSet();
            map.set({},1);set.add({});
            map.marker=42;set.marker=42;
            return [map.marker,set.marker,map.has({}),set.has({})].join("|");
        })()
// case: symbols / unregistered symbols are weak keys while registered symbols are rejected
(function(){
            var local=Symbol("local"),wellKnown=Symbol.iterator,registered=Symbol.for("registered");
            var map=new WeakMap(),set=new WeakSet();
            map.set(local,1).set(wellKnown,2);
            set.add(local).add(wellKnown);
            return [
                map.get(local),map.get(wellKnown),map.has(local),map.has(wellKnown),
                set.has(local),set.has(wellKnown),
                map.delete(local),set.delete(local),map.has(local),set.has(local),
                __completion(function(){return map.set(registered,3)}),
                __completion(function(){return set.add(registered)}),
                __completion(function(){return map.getOrInsert(registered,4)}),
                __completion(function(){return map.getOrInsertComputed(registered,function(){return 5})}),
                map.get(registered),map.has(registered),map.delete(registered),
                set.has(registered),set.delete(registered)
            ].join("|");
        })()
// case: constructor / constructors cache the adder and iterator next while observing access order
(function(){
            function weakMapRun(){
                var log="",key=Object(),custom=Object.create(WeakMap.prototype);
                var iterator=Object(),count=0,originalSet=WeakMap.prototype.set;
                var NewTarget=(function(){}).bind(null);
                Object.defineProperty(NewTarget,"prototype",{
                    configurable:true,get:function(){log+="prototype;";return custom}
                });
                Object.defineProperty(custom,"set",{
                    configurable:true,get:function(){
                        log+="set-get;";
                        return function(observedKey,value){
                            log+="adder:"+(observedKey===key)+":"+value+";";
                            Object.defineProperty(iterator,"next",{
                                configurable:true,writable:true,value:function(){throw "changed-next"}
                            });
                            return originalSet.call(this,observedKey,value);
                        };
                    }
                });
                var entry=Object();
                Object.defineProperty(entry,"0",{get:function(){log+="key;";return key}});
                Object.defineProperty(entry,"1",{get:function(){log+="value;";return 9}});
                var iterable=Object();
                Object.defineProperty(iterable,Symbol.iterator,{get:function(){
                    log+="iterator-get;";return function(){log+="iterator-call;";return iterator};
                }});
                Object.defineProperty(iterator,"next",{configurable:true,get:function(){
                    log+="next-get;";return function(){
                        log+="next-call;";var done=count++!==0,result=Object();
                        Object.defineProperty(result,"done",{get:function(){log+="done:"+done+";";return done}});
                        Object.defineProperty(result,"value",{get:function(){log+="step-value;";return entry}});
                        return result;
                    };
                }});
                var map=Reflect.construct(WeakMap,[iterable],NewTarget);
                function Fallback(){}Fallback.prototype=17;
                var fallback=Reflect.construct(WeakMap,[],Fallback);
                return [log,Object.getPrototypeOf(map)===custom,map.get(key),
                    Object.getPrototypeOf(fallback)===WeakMap.prototype,
                    __completion(function(){return WeakMap()})].join("|");
            }
            function weakSetRun(){
                var log="",key=Object(),custom=Object.create(WeakSet.prototype);
                var iterator=Object(),count=0,originalAdd=WeakSet.prototype.add;
                var NewTarget=(function(){}).bind(null);
                Object.defineProperty(NewTarget,"prototype",{
                    configurable:true,get:function(){log+="prototype;";return custom}
                });
                Object.defineProperty(custom,"add",{
                    configurable:true,get:function(){
                        log+="add-get;";
                        return function(observedKey){
                            log+="adder:"+(observedKey===key)+";";
                            Object.defineProperty(iterator,"next",{
                                configurable:true,writable:true,value:function(){throw "changed-next"}
                            });
                            return originalAdd.call(this,observedKey);
                        };
                    }
                });
                var iterable=Object();
                Object.defineProperty(iterable,Symbol.iterator,{get:function(){
                    log+="iterator-get;";return function(){log+="iterator-call;";return iterator};
                }});
                Object.defineProperty(iterator,"next",{configurable:true,get:function(){
                    log+="next-get;";return function(){
                        log+="next-call;";var done=count++!==0,result=Object();
                        Object.defineProperty(result,"done",{get:function(){log+="done:"+done+";";return done}});
                        Object.defineProperty(result,"value",{get:function(){log+="step-value;";return key}});
                        return result;
                    };
                }});
                var set=Reflect.construct(WeakSet,[iterable],NewTarget);
                function Fallback(){}Fallback.prototype=17;
                var fallback=Reflect.construct(WeakSet,[],Fallback);
                return [log,Object.getPrototypeOf(set)===custom,set.has(key),
                    Object.getPrototypeOf(fallback)===WeakSet.prototype,
                    __completion(function(){return WeakSet()})].join("|");
            }
            return weakMapRun()+"||"+weakSetRun();
        })()
// case: close / constructor element and adder failures close iterators at QuickJS boundaries
(function(){
            function mapRun(mode){
                var log="",iterator=Object(),iterable=Object(),count=0,key=Object();
                iterable[Symbol.iterator]=function(){log+="iterator;";return iterator};
                iterator.return=function(){log+="return;";throw "close"};
                iterator.next=function(){
                    log+="next;";
                    if(mode===4)throw "next";
                    var result=Object();
                    Object.defineProperty(result,"done",{get:function(){
                        log+="done;";if(mode===5)throw "done";return count++!==0;
                    }});
                    Object.defineProperty(result,"value",{get:function(){
                        log+="step-value;";if(mode===6)throw "step-value";
                        if(mode===0)return 1;
                        var entry=Object();
                        Object.defineProperty(entry,"0",{get:function(){
                            log+="key;";if(mode===2)throw "key";return mode===1?1:key;
                        }});
                        Object.defineProperty(entry,"1",{get:function(){log+="value;";return 7}});
                        return entry;
                    }});
                    return result;
                };
                var target=WeakMap;
                if(mode===3){
                    var prototype=Object.create(WeakMap.prototype);
                    prototype.set=function(){log+="adder;";throw "adder"};
                    target=function(){};target.prototype=prototype;
                }
                try{Reflect.construct(WeakMap,[iterable],target);return log+"missing"}
                catch(error){return log+"catch:"+(error!==null&&typeof error==="object"?error.name:String(error))}
            }
            function setRun(mode){
                var log="",iterator=Object(),iterable=Object(),count=0,key=Object();
                iterable[Symbol.iterator]=function(){log+="iterator;";return iterator};
                iterator.return=function(){log+="return;";throw "close"};
                iterator.next=function(){
                    log+="next;";if(mode===2)throw "next";
                    return {done:count++!==0,value:mode===0?1:key};
                };
                var target=WeakSet;
                if(mode===1){
                    var prototype=Object.create(WeakSet.prototype);
                    prototype.add=function(){log+="adder;";throw "adder"};
                    target=function(){};target.prototype=prototype;
                }
                try{Reflect.construct(WeakSet,[iterable],target);return log+"missing"}
                catch(error){return log+"catch:"+(error!==null&&typeof error==="object"?error.name:String(error))}
            }
            var output=[],index;
            for(index=0;index<7;index++)output[index]="m"+index+":"+mapRun(index);
            for(index=0;index<3;index++)output[7+index]="s"+index+":"+setRun(index);
            return output.join("|");
        })()
// case: upsert / WeakMap upsert methods validate and replace callback reentrant entries
(function(){
            var present=Object(),fresh=Object(),computedKey=Object(),side=Object(),thrown=Object();
            var invalid=Object(),map=new WeakMap(),called=0,callbackThis="unset",callbackKey="unset";
            map.set(present,undefined);
            var presentResult=map.getOrInsert(present,9);
            var inserted=map.getOrInsert(fresh,2);
            var computed=map.getOrInsertComputed(computedKey,function(key){
                "use strict";
                called++;callbackThis=this;callbackKey=key;
                map.set(computedKey,20);map.set(side,30);
                return 40;
            });
            var sentinel=Object(),sameThrow=false;
            try{
                map.getOrInsertComputed(thrown,function(){map.set(thrown,50);throw sentinel});
            }catch(error){sameThrow=error===sentinel}
            return [
                presentResult===undefined,inserted,called,callbackThis===undefined,
                callbackKey===computedKey,computed,map.get(computedKey),map.get(side),
                map.get(thrown),sameThrow,
                __completion(function(){return map.getOrInsertComputed(present,0)}),
                __completion(function(){return map.getOrInsertComputed(1,0)}),
                __completion(function(){return map.getOrInsertComputed(1,function(){return 1})}),
                __completion(function(){return map.getOrInsert(1,1)}),
                map.has(invalid)
            ].join("|");
        })()
// case: brands / prototype methods enforce weak collection brands and are not constructors
(function(){
            var key=Object(),map=new WeakMap(),set=new WeakSet();
            return [
                __completion(function(){return WeakMap.prototype.get.call(Object(),key)}),
                __completion(function(){return WeakMap.prototype.set.call(set,key,1)}),
                __completion(function(){return WeakSet.prototype.add.call(map,key)}),
                __completion(function(){return WeakSet.prototype.has.call(new Set(),key)}),
                __completion(function(){return new WeakMap.prototype.get()}),
                __completion(function(){return new WeakSet.prototype.add()}),
                __completion(function(){return WeakMap.prototype.set.call(map,key,1)===map}),
                __completion(function(){return WeakSet.prototype.add.call(set,key)===set}),
                __completion(function(){return WeakMap.prototype.getOrInsert.call(set,key,1)}),
                __completion(function(){return WeakMap.prototype.getOrInsertComputed.call(map,key,0)})
            ].join("|");
        })()
