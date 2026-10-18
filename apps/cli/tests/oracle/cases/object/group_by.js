// VALUE_CASES
// case: basic grouping uses a null-prototype object and genuine Arrays
(function(){
            var result=Object.groupBy([1,2,3,4],function(value,index){
                return value%2 ? "odd" : "even";
            });
            return (Object.getPrototypeOf(result)===null)+"|"+
                Object.getOwnPropertyNames(result).join(",")+"|"+
                result.odd.join(",")+"|"+result.even.join(",")+"|"+
                Array.isArray(result.odd)+"|"+
                (Object.getPrototypeOf(result.odd)===Array.prototype);
        })()
// case: property-key conversion supports symbols objects and proto spelling
(function(){
            var symbol=Symbol("bucket"),key=Object(),log="";
            key.toString=function(){log+="k";return "__proto__"};
            var result=Object.groupBy([1,2,3],function(value){
                return value===2 ? symbol : key;
            });
            var symbols=Object.getOwnPropertySymbols(result);
            return log+"|"+Object.getOwnPropertyNames(result).join(",")+"|"+
                (symbols.length===1&&symbols[0]===symbol)+"|"+
                result.__proto__.join(",")+"|"+result[symbol].join(",");
        })()
// case: string iteration groups Unicode code points rather than UTF-16 halves
(function(){
            var result=Object.groupBy("a𝄞a",function(value){return value});
            return Object.getOwnPropertyNames(result).join("|")+"|"+
                result.a.length+"|"+result["𝄞"].length+"|"+result["𝄞"][0];
        })()
// case: numeric property keys retain ordinary own-key ordering
(function(){
            var result=Object.groupBy([0,1,2,3],function(value){
                return value===0 ? 2 : value===1 ? 1 : value===2 ? -0 : "01";
            });
            return Object.getOwnPropertyNames(result).join("|")+"|"+
                result[0]+"|"+result[1]+"|"+result[2]+"|"+result["01"];
        })()
// case: callback receives the defining global object value and monotonic index
(function(){
            var log="";
            var result=Object.groupBy([4,5],function(value,index){
                "use strict";
                log+=(this===globalThis)+":"+value+":"+index+";";
                return "all";
            });
            return log+"|"+result.all.join(",");
        })()
// case: group properties are writable enumerable and configurable
(function(){
            var result=Object.groupBy([1],function(){return "x"});
            var enumerable=Object.prototype.propertyIsEnumerable.call(result,"x");
            result.x=9;
            var writable=result.x===9,deleted=delete result.x;
            return enumerable+"|"+writable+"|"+deleted+"|"+("x" in result);
        })()
// case: ordinary recursive callback reentry completes below the host-safe ceiling
(function(){
            var count=0;
            function group(depth){
                Object.groupBy([depth],function(){
                    count++;
                    if(depth)group(depth-1);
                    return "x";
                });
            }
            group(4);
            return count;
        })()
// ORDER_CASES
// case: callback validation precedes access to the iterable
(function(){
            var log="",iterable=Object();
            iterable.__defineGetter__(Symbol.iterator,function(){log+="iterator;";throw "touched"});
            try{Object.groupBy(iterable,0)}catch(error){
                return log+error.name+":"+error.message;
            }
            return "missing";
        })()
// case: iterator next is cached once and each step precedes callback and key conversion
(function(){
            var log="",count=0,iterable=Object(),iterator=Object(),key=Object();
            key.toString=function(){log+="key;";return "g"};
            iterable.__defineGetter__(Symbol.iterator,function(){
                log+="iterator-get;";
                return function(){log+="iterator-call;";return iterator};
            });
            iterator.__defineGetter__("next",function(){
                log+="next-get;";
                return function(){
                    log+="next-call;";
                    if(count++){
                        var finished=Object();finished.done=true;return finished;
                    }
                    var result=Object();
                    result.__defineGetter__("done",function(){log+="done;";return false});
                    result.__defineGetter__("value",function(){log+="value;";return 7});
                    return result;
                };
            });
            var grouped=Object.groupBy(iterable,function(value,index){
                log+="callback:"+value+":"+index+";";
                iterator.next=function(){throw "changed"};
                return key;
            });
            return log+"|"+grouped.g.join(",");
        })()
// case: callback and key conversion failures close while preserving the original throw
(function(){
            function run(mode){
                var log="",iterable=Object(),iterator=Object();
                iterable[Symbol.iterator]=function(){log+="iterator;";return iterator};
                iterator.next=function(){
                    log+="next;";
                    var result=Object();result.done=false;result.value=1;return result;
                };
                iterator.return=function(){log+="return;";throw "close"};
                try{
                    Object.groupBy(iterable,function(){
                        log+="callback;";
                        if(mode===0)throw "callback";
                        var key=Object();
                        key.toString=function(){log+="key;";throw "key"};
                        return key;
                    });
                }catch(error){return log+"throw:"+error}
                return "missing";
            }
            return run(0)+"|"+run(1);
        })()
// case: next done and value failures do not close the iterator
(function(){
            function run(mode){
                var log="",iterable=Object(),iterator=Object(),result=Object();
                iterable[Symbol.iterator]=function(){log+="iterator;";return iterator};
                iterator.next=function(){
                    log+="next;";
                    if(mode===0)throw "next";
                    return result;
                };
                iterator.return=function(){log+="return;"};
                result.__defineGetter__("done",function(){
                    log+="done;";if(mode===1)throw "done";return false;
                });
                result.__defineGetter__("value",function(){
                    log+="value;";throw "value";
                });
                try{Object.groupBy(iterable,function(){return "x"})}
                catch(error){return log+"throw:"+error}
                return "missing";
            }
            return run(0)+"|"+run(1)+"|"+run(2);
        })()
// case: a throwing next getter does not close the iterator
(function(){
            var log="",iterable=Object(),iterator=Object();
            iterable[Symbol.iterator]=function(){return iterator};
            iterator.__defineGetter__("next",function(){log+="next-get;";throw "next-get"});
            iterator.return=function(){log+="return;"};
            try{Object.groupBy(iterable,function(){return "x"})}
            catch(error){return log+"throw:"+error}
            return "missing";
        })()
// case: group append uses Array push Set semantics
(function(){
            var source=[9],log="";
            Array.prototype.__defineSetter__("0",function(value){log+="set0:"+value});
            try{
                var result=Object.groupBy(source,function(){return "x"});
                return result.x.length+"|"+result.x.hasOwnProperty("0")+"|"+
                    result.x[0]+"|"+log;
            }finally{delete Array.prototype[0]}
        })()
// case: an internal Array push failure does not close the iterator
(function(){
            var log="",iterable=Object(),iterator=Object(),steps=0;
            iterable[Symbol.iterator]=function(){return iterator};
            iterator.next=function(){
                log+="next;";
                var result=Object();result.done=steps++>0;result.value=9;return result;
            };
            iterator.return=function(){log+="return;"};
            try{
                Object.groupBy(iterable,function(){
                    log+="callback;";
                    var descriptor=Object();
                    descriptor.value=1;descriptor.writable=false;descriptor.configurable=true;
                    Object.defineProperty(Array.prototype,"0",descriptor);
                    return "x";
                });
            }catch(error){return log+"throw:"+error.name+":"+error.message}
            finally{delete Array.prototype[0]}
            return "missing";
        })()
// ERROR_CASES
// case: missing callback
Object.groupBy([])
// case: undefined iterable
Object.groupBy(undefined,function(){})
// case: null iterable
Object.groupBy(null,function(){})
// case: non-iterable primitive
Object.groupBy(1,function(){})
// case: non-callable iterator method
(function(){var value=Object();value[Symbol.iterator]=1;return Object.groupBy(value,function(){})})()
// case: iterator method returns a primitive
(function(){var value=Object();value[Symbol.iterator]=function(){return 1};return Object.groupBy(value,function(){})})()
// case: next is not callable
(function(){
            var value=Object();
            value[Symbol.iterator]=function(){var iterator=Object();iterator.next=1;return iterator};
            return Object.groupBy(value,function(){});
        })()
// case: next returns a primitive
(function(){
            var value=Object();
            value[Symbol.iterator]=function(){
                var iterator=Object();iterator.next=function(){return 1};return iterator;
            };
            return Object.groupBy(value,function(){});
        })()
// case: groupBy is not a constructor
new Object.groupBy([],function(){return 0})
