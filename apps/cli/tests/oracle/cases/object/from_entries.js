// VALUE_CASES
// case: basic entries create a fresh ordinary object with data properties
(function(){
            function bits(descriptor){
                return descriptor.writable+":"+descriptor.enumerable+":"+descriptor.configurable;
            }
            var first=Object.fromEntries.call(17,[["a",1],["b",2]]);
            var second=Object.fromEntries([["a",1]]);
            return (Object.getPrototypeOf(first)===Object.prototype)+"|"+
                (first!==second)+"|"+Object.getOwnPropertyNames(first).join(",")+"|"+
                first.a+":"+first.b+"|"+bits(Object.getOwnPropertyDescriptor(first,"a"));
        })()
// case: duplicate keys overwrite without moving while integer keys stay canonical
(function(){
            var result=Object.fromEntries([["z",1],[10,2],["2",3],["a",4],["z",5]]);
            return Object.getOwnPropertyNames(result).join(",")+"|"+
                result[2]+":"+result[10]+":"+result.z+":"+result.a;
        })()
// case: Symbol and proto-spelled keys are direct own data properties
(function(){
            var symbol=Symbol("entry"),payload=Object(),prototypeValue=Object();
            payload.kept=9;
            var result=Object.fromEntries([[symbol,payload],["__proto__",prototypeValue]]);
            var symbols=Object.getOwnPropertySymbols(result);
            var descriptor=Object.getOwnPropertyDescriptor(result,"__proto__");
            return (Object.getPrototypeOf(result)===Object.prototype)+"|"+
                (symbols.length===1&&symbols[0]===symbol)+"|"+(result[symbol]===payload)+"|"+
                (result.__proto__===prototypeValue)+"|"+result.hasOwnProperty("__proto__")+"|"+
                descriptor.writable+":"+descriptor.enumerable+":"+descriptor.configurable;
        })()
// case: each yielded entry may be any object with array-like zero and one properties
(function(){
            var arrayLike=Object();arrayLike[0]="arrayLike";arrayLike[1]=7;arrayLike[2]="ignored";
            var wrapped=Object("xy"),missing=["missing"],result;
            result=Object.fromEntries([wrapped,arrayLike,["extra",9,10],missing]);
            return Object.getOwnPropertyNames(result).join(",")+"|"+
                result.x+":"+result.arrayLike+":"+result.extra+":"+(result.missing===undefined);
        })()
// case: missing entry fields become undefined and numeric keys use ToPropertyKey
(function(){
            var empty=[],negativeZero=[-0,"zero"],nan=[NaN,"nan"];
            var result=Object.fromEntries([empty,negativeZero,nan]);
            return Object.getOwnPropertyNames(result).join(",")+"|"+
                (result.undefined===undefined)+":"+result[0]+":"+result.NaN;
        })()
// case: Map supplies its native entry iterator
(function(){
            var map=new Map([["x",1],["y",2]]),result=Object.fromEntries(map);
            return Object.getOwnPropertyNames(result).join(",")+":"+result.x+":"+result.y;
        })()
// case: Set values may themselves be entry objects
(function(){
            var set=new Set([["x",1],["y",2]]),result=Object.fromEntries(set);
            return Object.getOwnPropertyNames(result).join(",")+":"+result.x+":"+result.y;
        })()
// ORDER_CASES
// case: iterator acquisition caches next and reads done value zero one then key
(function(){
            var log="",calls=0,iterable=Object(),iterator=Object(),entry=Object(),key=Object();
            key[Symbol.toPrimitive]=function(hint){log+="key:"+hint+";";return "x"};
            entry.__defineGetter__("0",function(){log+="get-0;";return key});
            entry.__defineGetter__("1",function(){
                log+="get-1;";iterator.next=function(){throw "changed"};return 42;
            });
            iterable.__defineGetter__(Symbol.iterator,function(){
                log+="iterator-get;";
                return function(){log+="iterator-call;";return iterator};
            });
            iterator.__defineGetter__("next",function(){
                log+="next-get;";
                return function(){
                    log+="next-call;";
                    var step=Object();
                    if(calls++){
                        step.__defineGetter__("done",function(){log+="done:true;";return true});
                        step.__defineGetter__("value",function(){throw "finished value was read"});
                    }else{
                        step.__defineGetter__("done",function(){log+="done:false;";return false});
                        step.__defineGetter__("value",function(){log+="value;";return entry});
                    }
                    return step;
                };
            });
            iterator.return=function(){log+="return;"};
            var result=Object.fromEntries(iterable);
            return log+"|"+result.x+"|"+Object.getOwnPropertyNames(result).join(",");
        })()
// case: CreateDataProperty bypasses an inherited setter
(function(){
            var log="";
            Object.prototype.__defineSetter__("fromEntriesCreate",function(value){log+="set:"+value});
            try{
                var result=Object.fromEntries([["fromEntriesCreate",8]]);
                var descriptor=Object.getOwnPropertyDescriptor(result,"fromEntriesCreate");
                return log+"|"+result.fromEntriesCreate+"|"+result.hasOwnProperty("fromEntriesCreate")+"|"+
                    descriptor.writable+":"+descriptor.enumerable+":"+descriptor.configurable;
            }finally{delete Object.prototype.fromEntriesCreate}
        })()
// case: iterator acquisition failures occur before an iterator can be closed
(function(){
            function text(error){return typeof error+":"+error}
            function run(mode){
                var log="",iterable=Object();
                iterable.__defineGetter__(Symbol.iterator,function(){
                    log+="iterator-get;";
                    if(mode===0)throw "iterator-get";
                    return function(){log+="iterator-call;";if(mode===1)throw "iterator-call";return 1};
                });
                try{Object.fromEntries(iterable)}catch(error){return log+"throw:"+text(error)}
                return "missing";
            }
            return run(0)+"|"+run(1)+"|"+run(2);
        })()
// case: next getter call result done and value failures all close the iterator
(function(){
            function text(error){return typeof error+":"+error}
            function run(mode){
                var log="",iterable=Object(),iterator=Object(),step=Object();
                iterable[Symbol.iterator]=function(){log+="iterator;";return iterator};
                iterator.return=function(){log+="return;";throw "close"};
                if(mode===0){
                    iterator.__defineGetter__("next",function(){log+="next-get;";throw "next-get"});
                }else{
                    iterator.next=function(){
                        log+="next-call;";
                        if(mode===1)throw "next-call";
                        if(mode===2)return 1;
                        step.__defineGetter__("done",function(){
                            log+="done;";if(mode===3)throw "done";return false;
                        });
                        step.__defineGetter__("value",function(){
                            log+="value;";if(mode===4)throw "value";return 1;
                        });
                        return step;
                    };
                }
                try{Object.fromEntries(iterable)}catch(error){return log+"throw:"+text(error)}
                return "missing";
            }
            return run(0)+"|"+run(1)+"|"+run(2)+"|"+run(3)+"|"+run(4)+"|"+run(5);
        })()
// case: entry zero one and property-key failures close after completed reads
(function(){
            function text(error){return typeof error+":"+error}
            function run(mode){
                var log="",iterable=Object(),iterator=Object(),step=Object(),entry=Object(),key=Object();
                iterable[Symbol.iterator]=function(){return iterator};
                iterator.next=function(){step.done=false;step.value=entry;return step};
                iterator.return=function(){log+="return;";throw "close"};
                entry.__defineGetter__("0",function(){
                    log+="get-0;";if(mode===0)throw "get-0";return key;
                });
                entry.__defineGetter__("1",function(){
                    log+="get-1;";if(mode===1)throw "get-1";return 7;
                });
                key[Symbol.toPrimitive]=function(hint){
                    log+="key:"+hint+";";if(mode===2)throw "key";return Object();
                };
                try{Object.fromEntries(iterable)}catch(error){return log+"throw:"+text(error)}
                return "missing";
            }
            return run(0)+"|"+run(1)+"|"+run(2)+"|"+run(3);
        })()
// case: IteratorClose preserves the pending failure over return failures
(function(){
            function run(mode){
                var log="",iterable=Object(),iterator=Object(),step=Object();
                iterable[Symbol.iterator]=function(){return iterator};
                iterator.next=function(){step.done=false;step.value=1;return step};
                if(mode===0)iterator.__defineGetter__("return",function(){log+="return-get;";throw "close-get"});
                if(mode===1)iterator.return=function(){log+="return-call;";throw "close-call"};
                if(mode===2)iterator.return=1;
                if(mode===3)iterator.return=function(){log+="return-call;";return 1};
                try{Object.fromEntries(iterable)}catch(error){return log+error.name+":"+error.message}
                return "missing";
            }
            return run(0)+"|"+run(1)+"|"+run(2)+"|"+run(3)+"|"+run(4);
        })()
// case: generator finally runs when a non-object entry triggers IteratorClose
(function(){
            var log="";
            function* entries(){try{yield ["a",1];yield 2}finally{log+="finally;"}}
            try{Object.fromEntries(entries())}catch(error){return log+error.name+":"+error.message}
            return "missing";
        })()
// ERROR_CASES
// case: missing iterable
Object.fromEntries()
// case: undefined iterable
Object.fromEntries(undefined)
// case: null iterable
Object.fromEntries(null)
// case: non-iterable number
Object.fromEntries(1)
// case: non-callable iterator method
(function(){var value=Object();value[Symbol.iterator]=1;return Object.fromEntries(value)})()
// case: iterator method returns a primitive
(function(){var value=Object();value[Symbol.iterator]=function(){return 1};return Object.fromEntries(value)})()
// case: next is not callable
(function(){
            var value=Object();
            value[Symbol.iterator]=function(){var iterator=Object();iterator.next=1;return iterator};
            return Object.fromEntries(value);
        })()
// case: next returns a primitive
(function(){
            var value=Object();
            value[Symbol.iterator]=function(){
                var iterator=Object();iterator.next=function(){return 1};return iterator;
            };
            return Object.fromEntries(value);
        })()
// case: undefined entry
Object.fromEntries([undefined])
// case: null entry
Object.fromEntries([null])
// case: numeric entry
Object.fromEntries([1])
// case: String iterator yields primitive entries
Object.fromEntries("ab")
// case: entry key cannot become a property key
(function(){var key=Object();key[Symbol.toPrimitive]=function(){return Object()};return Object.fromEntries([[key,1]])})()
// case: fromEntries is not a constructor
new Object.fromEntries([])
// EXOTIC_CASES
// case: Proxy entries expose zero and one Get trap ordering
(function(){
            var log="",base=["x",7];
            var entry=new Proxy(base,{get:function(target,key,receiver){
                log+="get:"+String(key)+";";return Reflect.get(target,key,receiver);
            }});
            var result=Object.fromEntries([entry]);
            return log+result.x;
        })()
// case: TypedArray entries expose their integer indexed zero and one values
(function(){
            var result=Object.fromEntries([new Uint8Array([120,7])]);
            return Object.getOwnPropertyNames(result).join(",")+"|"+result[120];
        })()
// case: Proxy entry abrupt completion closes the iterator and preserves the throw
(function(){
            var log="",entry=new Proxy(["x",7],{get:function(target,key,receiver){
                log+="get:"+String(key)+";";
                if(key==="0")throw "trap";
                return Reflect.get(target,key,receiver);
            }}),done=false,iterator={
                next:function(){if(done)return {done:true};done=true;return {done:false,value:entry}},
                return:function(){log+="return;";return {}}
            },iterable={};
            iterable[Symbol.iterator]=function(){return iterator};
            try{Object.fromEntries(iterable)}catch(error){return log+error}
            return "missing";
        })()
