// case: surface descriptors and constructor receiver validation
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
            var property=Object.getOwnPropertyDescriptor(base,"of");
            var length=Object.getOwnPropertyDescriptor(base.of,"length");
            var name=Object.getOwnPropertyDescriptor(base.of,"name");
            var unbound=base.of;
            var method={m(){}}.m;
            return [
                base===Object.getPrototypeOf(Float64Array),
                base.of===Uint8Array.of,
                Uint8Array.hasOwnProperty("of"),
                property.writable,property.enumerable,property.configurable,
                base.of.name,base.of.length,isConstructor(base.of),
                length.writable,length.enumerable,length.configurable,
                name.writable,name.enumerable,name.configurable,
                completion(function(){return unbound()}),
                completion(function(){return base.of.call(null)}),
                completion(function(){return base.of.call(false)}),
                completion(function(){return base.of.call(1)}),
                completion(function(){return base.of.call("x")}),
                completion(function(){return base.of.call(1n)}),
                completion(function(){return base.of.call(Symbol("receiver"))}),
                completion(function(){return base.of()}),
                completion(function(){return base.of.call({})}),
                completion(function(){return base.of.call(method)}),
                completion(function(){return new base.of()})
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
            var numbers=[
                Int8Array,Uint8Array,Uint8ClampedArray,
                Int16Array,Uint16Array,Int32Array,Uint32Array,
                Float16Array,Float32Array,Float64Array
            ];
            var out=numbers.map(function(C){
                return C.name+":"+
                    values(C.of(-1,257,2.5,3.5,NaN,Infinity,-0));
            });
            var bigintObject={valueOf:function(){return 5n}};
            out.push(
                "BigInt64Array:"+
                    values(BigInt64Array.of(
                        -1n,18446744073709551617n,"2",true,bigintObject)),
                "BigUint64Array:"+
                    values(BigUint64Array.of(
                        -1n,18446744073709551617n,"2",true,bigintObject))
            );
            return out.join("|");
        })()
// case: static from shares only the direct constructor diagnostic seam
(function(){
            function completion(thunk){
                try{return "return:"+String(thunk())}
                catch(error){return "throw:"+error.name+":"+error.message}
            }
            var base=Object.getPrototypeOf(Uint8Array),log="";
            var arrayLike={};
            Object.defineProperty(arrayLike,Symbol.iterator,{
                get:function(){log+="I";return undefined}
            });
            Object.defineProperty(arrayLike,"length",{
                get:function(){log+="L";return 0}
            });
            var arrayLikeResult=completion(function(){
                return base.from.call(1,arrayLike);
            });
            var arrayLikeLog=log;

            log="";
            var iterable={};
            Object.defineProperty(iterable,Symbol.iterator,{
                get:function(){
                    log+="G";
                    return function(){
                        log+="I";
                        return {
                            next:function(){
                                log+="N";
                                return {done:true};
                            }
                        };
                    };
                }
            });
            var iterableResult=completion(function(){
                return base.from.call(false,iterable);
            });
            var iterableLog=log;

            log="";
            var mapResult=completion(function(){
                return base.from.call({},arrayLike,1);
            });
            var mapLog=log;

            var source=new Uint8Array([1]);
            source.constructor={};
            source.constructor[Symbol.species]=1;
            var speciesResult=completion(function(){
                return source.slice(0);
            });
            return [
                arrayLikeResult,arrayLikeLog,
                iterableResult,iterableLog,
                mapResult,mapLog,speciesResult
            ].join("|");
        })()
// case: custom constructors newTarget and result validation
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
            var result=base.of.call(Custom,1,65538);

            function Wrapper(length){
                log+="W"+(new.target===Wrapper)+";";
                return Reflect.construct(Uint8Array,[length],new.target);
            }
            Object.setPrototypeOf(Wrapper,base);
            Wrapper.prototype=Object.create(Uint8Array.prototype);
            Object.defineProperty(
                Wrapper.prototype,"constructor",
                {value:Wrapper,writable:true,configurable:true}
            );
            var wrapped=Wrapper.of(9,10);

            function BoundTarget(length){
                log+="B"+(new.target===BoundTarget)+";";
                return new Uint8Array(length);
            }
            var Bound=BoundTarget.bind(null);
            var bound=base.of.call(Bound,3,4);

            var proxyLog="",proxy;
            function ProxyTarget(){}
            proxy=new Proxy(ProxyTarget,{
                construct:function(target,args,newTarget){
                    proxyLog=
                        args.length+":"+args[0]+":"+(newTarget===proxy);
                    return new Uint8Array(args[0]);
                }
            });
            var proxied=base.of.call(proxy,5,6,7);

            var bigintTarget;
            function BigintTarget(length){
                bigintTarget=new BigInt64Array(length);
                return bigintTarget;
            }
            var bigintMismatch=completion(function(){
                return Uint8Array.of.call(BigintTarget,1);
            });
            var numberTarget;
            function NumberTarget(length){
                numberTarget=new Uint8Array(length);
                return numberTarget;
            }
            var numberMismatch=BigInt64Array.of.call(NumberTarget,257);

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
            var proxyTarget=new Uint8Array(2);
            function ProxyResult(){return new Proxy(proxyTarget,{})}
            function Abrupt(){throw token}

            return [
                log,
                result===target,result.length,result[0],result[1],result[2],
                Object.getPrototypeOf(wrapped)===Wrapper.prototype,
                wrapped.length,wrapped[0],wrapped[1],
                bound.length,bound[0],bound[1],
                proxyLog,proxied.length,proxied[0],proxied[1],proxied[2],
                bigintMismatch,String(bigintTarget[0]),
                numberMismatch===numberTarget,
                numberTarget.length,numberTarget[0],
                completion(function(){return base.of.call(Ordinary,1)}),
                completion(function(){return base.of.call(ExplicitOrdinary,1)}),
                completion(function(){return base.of.call(Short,1,2)}),
                completion(function(){return base.of.call(Detached,1)}),
                completion(function(){return base.of.call(OutOfBounds,1)}),
                completion(function(){return base.of.call(ProxyResult,1,2)}),
                completion(function(){return base.of.call(Abrupt,1)})
            ].join("|");
        })()
// case: constructor conversion and abrupt completion ordering
(function(){
            var base=Object.getPrototypeOf(Uint8Array);
            var log="",target,token={},ctorToken={};
            function Custom(length){
                log+="C"+arguments.length+":"+length+";";
                target=new Uint8Array(length);
                return target;
            }
            var first={
                valueOf:function(){log+="A"+target[0]+";";return 7}
            };
            var second={
                valueOf:function(){log+="B"+target[0]+";";throw token}
            };
            var third={
                valueOf:function(){log+="D";return 9}
            };
            var same=false;
            try{base.of.call(Custom,first,second,third)}
            catch(error){same=error===token}
            var partial=target[0]+","+target[1]+","+target[2];

            var untouched={
                valueOf:function(){log+="I";return 1}
            };
            function AbruptConstructor(){
                log+="X;";
                throw ctorToken;
            }
            var ctorSame=false;
            try{base.of.call(AbruptConstructor,untouched)}
            catch(error){ctorSame=error===ctorToken}

            var bigintTarget;
            function BigintResult(length){
                bigintTarget=new BigInt64Array(length);
                return bigintTarget;
            }
            var later={
                valueOf:function(){log+="L";return 3n}
            };
            var bigintError;
            try{base.of.call(BigintResult,1n,2,later);bigintError="missing"}
            catch(error){bigintError=error.name}
            return [
                log,same,partial,ctorSame,bigintError,
                String(bigintTarget[0]),String(bigintTarget[1]),
                String(bigintTarget[2])
            ].join("|");
        })()
// case: RAB shrink grow detach and OOB writes
(function(){
            var base=Object.getPrototypeOf(Uint8Array),out=[],log="";

            var buffer=new ArrayBuffer(3,{maxByteLength:4});
            var target=new Int8Array(buffer);
            function TrackingResult(){return target}
            var one={
                valueOf:function(){log+="a";buffer.resize(0);return 1}
            };
            var two={
                valueOf:function(){log+="b";buffer.resize(4);return 2}
            };
            var result=base.of.call(TrackingResult,one,two,3);
            out.push(
                result===target,target.length,
                target[0],target[1],target[2],target[3]
            );

            buffer=new ArrayBuffer(6,{maxByteLength:8});
            target=new Uint16Array(buffer,2,2);
            function FixedResult(){return target}
            one={
                valueOf:function(){log+="c";buffer.resize(2);return 11}
            };
            two={
                valueOf:function(){log+="d";buffer.resize(6);return 22}
            };
            result=base.of.call(FixedResult,one,two);
            out.push(result===target,target.length,target[0],target[1]);

            buffer=new ArrayBuffer(3);
            target=new Uint8Array(buffer);
            function DetachedResult(){return target}
            one={
                valueOf:function(){
                    log+="e";
                    buffer.transfer();
                    return 31;
                }
            };
            two={valueOf:function(){log+="f";return 32}};
            var three={valueOf:function(){log+="g";return 33}};
            result=base.of.call(DetachedResult,one,two,three);
            out.push(
                result===target,buffer.detached,target.length,
                String(target[0]),String(target[1]),String(target[2])
            );
            return log+"|"+out.join(",");
        })()
// case: zero and safe large argument counts
(function(){
            var base=Object.getPrototypeOf(Uint8Array),log="";
            function Empty(length){
                log+=arguments.length+":"+length+":"+(new.target===Empty);
                return new Uint8Array(length);
            }
            var empty=base.of.call(Empty);
            var input=[],i;
            for(i=0;i<512;i++)input[i]=i;
            var large=Uint8Array.of.apply(Uint8Array,input);
            var sum=0;
            for(i=0;i<large.length;i+=17)sum+=large[i];
            return [
                log,empty.length,large.length,
                large[0],large[255],large[256],large[511],sum
            ].join("|");
        })()
