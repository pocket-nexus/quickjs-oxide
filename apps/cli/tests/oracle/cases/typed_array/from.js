// case: surface descriptors nullish source and receiver validation
(function(){
            function completion(thunk){
                try{return "return:"+String(thunk())}
                catch(error){return "throw:"+error.name+":"+error.message}
            }
            function isConstructor(value){
                try{Reflect.construct(function(){},[],value);return true}
                catch(_error){return false}
            }
            var base=Object.getPrototypeOf(Uint8Array);
            var property=Object.getOwnPropertyDescriptor(base,"from");
            var length=Object.getOwnPropertyDescriptor(base.from,"length");
            var name=Object.getOwnPropertyDescriptor(base.from,"name");
            var unbound=base.from;
            return [
                base===Object.getPrototypeOf(Float64Array),
                base.from===Uint8Array.from,
                Uint8Array.hasOwnProperty("from"),
                property.writable,property.enumerable,property.configurable,
                base.from.name,base.from.length,isConstructor(base.from),
                length.writable,length.enumerable,length.configurable,
                name.writable,name.enumerable,name.configurable,
                completion(function(){return base.from.call(Uint8Array,undefined)}),
                completion(function(){return base.from.call(Uint8Array,null)}),
                completion(function(){return base.from()}),
                completion(function(){return unbound([])}),
                completion(function(){return base.from.call(1,[])}),
                completion(function(){return base.from.call({},[])}),
                completion(function(){return base.from.call({},null,1)}),
                completion(function(){return new base.from([])})
            ].join("|");
        })()
// case: all twelve concrete classes and Number BigInt conversion
(function(){
            function encode(value){
                if(typeof value==="bigint")return String(value)+"n";
                if(value!==value)return "NaN";
                if(value===Infinity)return "+Infinity";
                if(value===-Infinity)return "-Infinity";
                if(value===0 && 1/value<0)return "-0";
                return String(value);
            }
            function values(array){
                var out=[];
                for(var i=0;i<array.length;i++)out.push(encode(array[i]));
                return out.join(",");
            }
            var numericSource={
                0:-1,1:257,2:2.5,3:3.5,4:NaN,5:Infinity,6:-0,
                length:7
            };
            numericSource[Symbol.iterator]=null;
            var numbers=[
                Int8Array,Uint8Array,Uint8ClampedArray,
                Int16Array,Uint16Array,Int32Array,Uint32Array,
                Float16Array,Float32Array,Float64Array
            ];
            var out=numbers.map(function(C){
                return C.name+":"+values(C.from(numericSource));
            });
            var bigintObject={valueOf:function(){return 5n}};
            var bigintSource=[
                -1n,18446744073709551617n,"2",true,bigintObject
            ];
            out.push(
                "BigInt64Array:"+values(BigInt64Array.from(bigintSource)),
                "BigUint64Array:"+values(BigUint64Array.from(bigintSource))
            );
            return out.join("|");
        })()
// case: iterable materialization cached next and map write order
(function(){
            var base=Object.getPrototypeOf(Uint8Array);
            var log="",step=0,target,mapThis={tag:"map-this"};
            var source={};
            Object.defineProperty(source,Symbol.iterator,{
                get:function(){
                    log+="G";
                    return function(){
                        log+="I"+(this===source);
                        var iterator={};
                        Object.defineProperty(iterator,"next",{
                            get:function(){
                                log+="X";
                                return function(){
                                    log+="N"+(this===iterator);
                                    step++;
                                    var current=step,result={};
                                    Object.defineProperty(result,"done",{
                                        get:function(){
                                            log+=(current>2?"E":"D");
                                            return current>2;
                                        }
                                    });
                                    Object.defineProperty(result,"value",{
                                        get:function(){
                                            log+=(current>2?"Q":"V");
                                            return current===1 ? 10 : 20;
                                        }
                                    });
                                    return result;
                                };
                            }
                        });
                        iterator.return=function(){log+="R";return {done:true}};
                        return iterator;
                    };
                }
            });
            function Result(length){
                log+="C"+arguments.length+":"+length+":"+
                    (new.target===Result);
                target=new Uint8Array(length);
                return target;
            }
            function mapper(value,index){
                log+="M"+index+":"+(this===mapThis);
                return {
                    valueOf:function(){
                        log+="T"+value;
                        return value+index;
                    }
                };
            }
            var result=base.from.call(Result,source,mapper,mapThis);
            return [
                log,result===target,target.length,target[0],target[1]
            ].join("|");
        })()
// case: array-like length construction property map and conversion order
(function(){
            function completion(thunk){
                try{return "return:"+String(thunk())}
                catch(error){return "throw:"+error.name+":"+error.message}
            }
            var base=Object.getPrototypeOf(Uint8Array);
            var log="",target,mapThis={};
            var source={};
            Object.defineProperty(source,Symbol.iterator,{
                get:function(){log+="G";return null}
            });
            Object.defineProperty(source,"length",{
                get:function(){
                    log+="L";
                    return {valueOf:function(){log+="T";return 2.9}};
                }
            });
            Object.defineProperty(source,"0",{
                get:function(){log+="A";return 5}
            });
            Object.defineProperty(source,"1",{
                get:function(){log+="B";return 6}
            });
            function Result(length){
                log+="C"+length;
                target=new Uint8Array(length);
                return target;
            }
            function mapper(value,index){
                log+="M"+index+":"+(this===mapThis);
                return {
                    valueOf:function(){log+="V"+value;return value+index}
                };
            }
            var result=base.from.call(Result,source,mapper,mapThis);

            var untouched="",invalid={};
            Object.defineProperty(invalid,Symbol.iterator,{
                get:function(){untouched+="I";return null}
            });
            var invalidMap=completion(function(){
                return base.from.call({},invalid,1);
            });
            var explicitUndefined=Uint8Array.from(
                {0:9,length:1,[Symbol.iterator]:null},
                undefined
            );
            return [
                log,result===target,target.length,target[0],target[1],
                invalidMap,untouched,
                explicitUndefined.length,explicitUndefined[0],
                Uint8Array.from(1).length,
                Uint8Array.from(false).length,
                Uint8Array.from(Symbol("source")).length
            ].join("|");
        })()
// case: custom bound Proxy constructors and actual result element type
(function(){
            function completion(thunk){
                try{return "return:"+String(thunk())}
                catch(error){
                    if(error===token)return "throw:token";
                    return "throw:"+error.name+":"+error.message;
                }
            }
            var base=Object.getPrototypeOf(Uint8Array),log="",target,token={};
            function Custom(length){
                log+="C"+arguments.length+":"+length+":"+
                    (new.target===Custom)+";";
                target=new Uint16Array(length+1);
                return target;
            }
            var result=base.from.call(Custom,[1,65538]);

            function BoundTarget(length){
                log+="B"+(new.target===BoundTarget)+";";
                return new Uint8Array(length);
            }
            var Bound=BoundTarget.bind(null);
            var bound=base.from.call(Bound,[3,4]);

            var proxyLog="",proxy;
            function ProxyTarget(){}
            proxy=new Proxy(ProxyTarget,{
                construct:function(target,args,newTarget){
                    proxyLog=
                        args.length+":"+args[0]+":"+(newTarget===proxy);
                    return new Uint8Array(args[0]);
                }
            });
            var proxied=base.from.call(proxy,[5,6,7]);

            var bigintTarget;
            function BigintTarget(length){
                bigintTarget=new BigInt64Array(length);
                return bigintTarget;
            }
            var bigintMismatch=completion(function(){
                return Uint8Array.from.call(BigintTarget,[1]);
            });
            var numberTarget;
            function NumberTarget(length){
                numberTarget=new Uint8Array(length);
                return numberTarget;
            }
            var numberMismatch=BigInt64Array.from.call(NumberTarget,[257]);

            function Ordinary(){}
            function ExplicitOrdinary(){return {}}
            function Short(){return new Uint8Array(1)}
            function Detached(length){
                var buffer=new ArrayBuffer(length);
                var value=new Uint8Array(buffer);
                buffer.transfer();
                return value;
            }
            function OutOfBounds(length){
                var buffer=new ArrayBuffer(length,{maxByteLength:length});
                var value=new Uint8Array(buffer,0,length);
                buffer.resize(0);
                return value;
            }
            function Abrupt(){throw token}
            var materialized=0,observed={};
            observed[Symbol.iterator]=function(){
                return {
                    next:function(){
                        materialized++;
                        return {done:true};
                    }
                };
            };

            return [
                log,
                result===target,result.length,result[0],result[1],result[2],
                bound.length,bound[0],bound[1],
                proxyLog,proxied.length,proxied[0],proxied[1],proxied[2],
                bigintMismatch,String(bigintTarget[0]),
                numberMismatch===numberTarget,
                numberTarget.length,numberTarget[0],
                completion(function(){return base.from.call(Ordinary,[1])}),
                completion(function(){return base.from.call(ExplicitOrdinary,[1])}),
                completion(function(){return base.from.call(Short,[1,2])}),
                completion(function(){return base.from.call(Detached,[1])}),
                completion(function(){return base.from.call(OutOfBounds,[1])}),
                completion(function(){return base.from.call(Abrupt,[1])}),
                completion(function(){return base.from.call({},observed)}),
                materialized
            ].join("|");
        })()
// case: iterator abrupt identity no-close behavior and partial map writes
(function(){
            var base=Object.getPrototypeOf(Uint8Array),token={};
            function completion(thunk){
                try{return "return:"+String(thunk())}
                catch(error){
                    if(error===token)return "throw:token";
                    return "throw:"+error.name+":"+error.message;
                }
            }
            function probe(mode){
                var log="",source={},iterator={};
                iterator.return=function(){log+="R";return {done:true}};
                if(mode==="next-get"){
                    Object.defineProperty(iterator,"next",{
                        get:function(){log+="X";throw token}
                    });
                }else{
                    iterator.next=function(){
                        log+="N";
                        if(mode==="next-result")return 1;
                        var result={};
                        Object.defineProperty(result,"done",{
                            get:function(){
                                log+="D";
                                if(mode==="done")throw token;
                                return false;
                            }
                        });
                        Object.defineProperty(result,"value",{
                            get:function(){log+="V";throw token}
                        });
                        return result;
                    };
                }
                source[Symbol.iterator]=function(){log+="I";return iterator};
                var result=completion(function(){
                    return base.from.call(Uint8Array,source);
                });
                return mode+":"+result+":"+log;
            }
            var getterLog="",getterSource={};
            Object.defineProperty(getterSource,Symbol.iterator,{
                get:function(){getterLog+="G";throw token}
            });
            var getterResult=completion(function(){
                return base.from.call(Uint8Array,getterSource);
            });

            var primitiveLog="",primitiveSource={};
            primitiveSource[Symbol.iterator]=function(){
                primitiveLog+="I";
                return 1;
            };
            var primitiveResult=completion(function(){
                return base.from.call(Uint8Array,primitiveSource);
            });

            var constructorLog="",constructorSource={};
            constructorSource[Symbol.iterator]=function(){
                var done=false;
                return {
                    next:function(){
                        constructorLog+="N";
                        if(done)return {done:true};
                        done=true;
                        return {done:false,value:1};
                    },
                    return:function(){
                        constructorLog+="R";
                        return {done:true};
                    }
                };
            };
            function AbruptConstructor(){constructorLog+="C";throw token}
            var constructorResult=completion(function(){
                return base.from.call(AbruptConstructor,constructorSource);
            });

            var mapLog="",mapTarget,mapSource={};
            mapSource[Symbol.iterator]=function(){
                var index=0;
                return {
                    next:function(){
                        mapLog+="N";
                        index++;
                        return index<=3
                            ? {done:false,value:index}
                            : {done:true};
                    },
                    return:function(){mapLog+="R";return {done:true}}
                };
            };
            function MapTarget(length){
                mapLog+="C";
                mapTarget=new Uint8Array(length);
                return mapTarget;
            }
            function mapper(value,index){
                mapLog+="M"+index;
                if(index===1)throw token;
                return value+9;
            }
            var mapResult=completion(function(){
                return base.from.call(MapTarget,mapSource,mapper);
            });
            return [
                getterResult,getterLog,
                primitiveResult,primitiveLog,
                probe("next-get"),
                probe("next-result"),
                probe("done"),
                probe("value"),
                constructorResult,constructorLog,
                mapResult,mapLog,
                mapTarget[0],mapTarget[1],mapTarget[2]
            ].join("|");
        })()
// case: RAB shrink grow detach OOB and conversion writes
(function(){
            var base=Object.getPrototypeOf(Uint8Array),out=[],log="";

            var buffer=new ArrayBuffer(3,{maxByteLength:4});
            var target=new Int8Array(buffer);
            function TrackingResult(){return target}
            function trackingMap(value,index){
                log+="M"+index;
                return {
                    valueOf:function(){
                        log+="V"+index;
                        if(index===0)buffer.resize(0);
                        if(index===1)buffer.resize(4);
                        return value;
                    }
                };
            }
            var result=base.from.call(
                TrackingResult,[1,2,3],trackingMap
            );
            out.push(
                result===target,target.length,
                target[0],target[1],target[2],target[3]
            );

            buffer=new ArrayBuffer(6,{maxByteLength:8});
            target=new Uint16Array(buffer,2,2);
            function FixedResult(){return target}
            function fixedMap(value,index){
                log+="F"+index;
                return {
                    valueOf:function(){
                        log+="W"+index;
                        buffer.resize(index===0 ? 2 : 6);
                        return value;
                    }
                };
            }
            result=base.from.call(FixedResult,[11,22],fixedMap);
            out.push(result===target,target.length,target[0],target[1]);

            buffer=new ArrayBuffer(3);
            target=new Uint8Array(buffer);
            function DetachedResult(){return target}
            function detachedMap(value,index){
                log+="D"+index;
                return {
                    valueOf:function(){
                        log+="T"+index;
                        if(index===0)buffer.transfer();
                        return value;
                    }
                };
            }
            result=base.from.call(
                DetachedResult,[31,32,33],detachedMap
            );
            out.push(
                result===target,buffer.detached,target.length,
                String(target[0]),String(target[1]),String(target[2])
            );
            return log+"|"+out.join(",");
        })()
// case: zero and safe large iterable and array-like inputs
(function(){
            var base=Object.getPrototypeOf(Uint8Array),log="";
            function Empty(length){
                log+=arguments.length+":"+length+":"+(new.target===Empty);
                return new Uint8Array(length);
            }
            var emptyIterable=base.from.call(Empty,[]);
            var emptyArrayLike=base.from.call(
                Empty,{length:-1,[Symbol.iterator]:null}
            );
            var input=[],i;
            for(i=0;i<512;i++)input[i]=i;
            var large=Uint8Array.from(input,function(value,index){
                return value+(index&1);
            });
            var sum=0;
            for(i=0;i<large.length;i+=17)sum+=large[i];
            return [
                log,emptyIterable.length,emptyArrayLike.length,
                large.length,large[0],large[1],
                large[255],large[256],large[511],sum
            ].join("|");
        })()
