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
function __completion(thunk){
    try{return "return:"+String(thunk())}
    catch(error){return "throw:"+error.name+":"+error.message}
}
function __keys(object){return Reflect.ownKeys(object).map(String).join(",")}
function __metadata(owner,key){
    var fn=owner[key];
    return String(key)+":"+fn.name+":"+fn.length+":"+__isConstructor(fn)+":"+
        __bits(owner,key)+":"+__keys(fn);
}
// case: constructor prototype descriptors and method graph
(function(){
            var byteDescriptor=Object.getOwnPropertyDescriptor(
                ArrayBuffer.prototype,"byteLength");
            var speciesDescriptor=Object.getOwnPropertyDescriptor(
                ArrayBuffer,Symbol.species);
            return [
                "global="+__bits(globalThis,"ArrayBuffer"),
                "ctor="+ArrayBuffer.name+":"+ArrayBuffer.length+":"+
                    __isConstructor(ArrayBuffer)+":"+__keys(ArrayBuffer),
                "links="+(Object.getPrototypeOf(ArrayBuffer)===Function.prototype)+":"+
                    (Object.getPrototypeOf(ArrayBuffer.prototype)===Object.prototype)+":"+
                    (ArrayBuffer.prototype.constructor===ArrayBuffer),
                "proto="+__keys(ArrayBuffer.prototype),
                "methods="+["resize","slice","transfer","transferToFixedLength"].
                    map(function(key){return __metadata(ArrayBuffer.prototype,key)}).join(";"),
                "byte="+__bits(ArrayBuffer.prototype,"byteLength")+":"+
                    byteDescriptor.get.name+":"+byteDescriptor.get.length+":"+
                    __isConstructor(byteDescriptor.get),
                "species="+__bits(ArrayBuffer,Symbol.species)+":"+
                    speciesDescriptor.get.name+":"+speciesDescriptor.get.length+":"+
                    speciesDescriptor.get.call(7),
                "tag="+__bits(ArrayBuffer.prototype,Symbol.toStringTag)+":"+
                    ArrayBuffer.prototype[Symbol.toStringTag],
                "view="+ArrayBuffer.isView(new ArrayBuffer(0))+":"+
                    ArrayBuffer.isView({})
            ].join("|");
        })()
// case: constructor coercion newTarget and QuickJS Int64 quirks
(function(){
            var log="",custom=Object.create(ArrayBuffer.prototype);
            var NewTarget=(function(){}).bind(null);
            Object.defineProperty(NewTarget,"prototype",{
                get:function(){log+="p";return custom}
            });
            var length={valueOf:function(){log+="l";return 3.9}};
            var maximum={valueOf:function(){log+="v";return 7.9}};
            var options={};
            Object.defineProperty(options,"maxByteLength",{
                get:function(){log+="m";return maximum}
            });
            var buffer=Reflect.construct(ArrayBuffer,[length,options],NewTarget);
            var negative=__completion(function(){
                return Reflect.construct(
                    ArrayBuffer,[0,{maxByteLength:-1}],NewTarget);
            });
            var infinity=new ArrayBuffer(0,{maxByteLength:Infinity});
            var wrapped=new ArrayBuffer(
                0,{maxByteLength:18446744073709551616});
            return [
                log,Object.getPrototypeOf(buffer)===custom,
                buffer.byteLength,buffer.maxByteLength,buffer.resizable,
                negative,infinity.maxByteLength,infinity.resizable,
                wrapped.maxByteLength,wrapped.resizable,
                new ArrayBuffer(2,1).byteLength
            ].join("|");
        })()
// case: resize transfer and fixed-length transfer state
(function(){
            var fixed=new ArrayBuffer(4);
            var resizable=new ArrayBuffer(4,{maxByteLength:8}),log="";
            resizable.constructor={
                get [Symbol.species](){log+="species";throw 1}
            };
            resizable.resize({
                valueOf:function(){log+="resize";return 6}
            });
            var moved=resizable.transfer({
                valueOf:function(){log+="transfer";return 7}
            });
            var fixedMoved=moved.transferToFixedLength(3);
            return [
                log,resizable.detached,resizable.byteLength,
                resizable.maxByteLength,resizable.resizable,
                moved.detached,moved.byteLength,moved.maxByteLength,moved.resizable,
                fixedMoved.byteLength,fixedMoved.maxByteLength,fixedMoved.resizable,
                moved.detached,fixed.transfer().resizable
            ].join("|");
        })()
// case: slice species ordering and post-constructor revalidation
(function(){
            var log="",source=new ArrayBuffer(8,{maxByteLength:12});
            var start={valueOf:function(){log+="s";return 2}};
            var end={valueOf:function(){log+="e";return 6}};
            source.constructor={
                get [Symbol.species](){
                    log+="g";
                    return function(length){
                        log+="c"+length;
                        return new ArrayBuffer(length+1);
                    };
                }
            };
            var target=source.slice(start,end);
            var same=new ArrayBuffer(2);
            same.constructor={
                get [Symbol.species](){return function(){return same}}
            };
            var detached=new ArrayBuffer(4);
            detached.constructor={
                get [Symbol.species](){
                    return function(){
                        detached.transfer();
                        return new ArrayBuffer(4);
                    };
                }
            };
            var shrunk=new ArrayBuffer(8,{maxByteLength:8});
            shrunk.constructor={
                get [Symbol.species](){
                    return function(){
                        shrunk.resize(1);
                        return new ArrayBuffer(8);
                    };
                }
            };
            return [
                log,target.byteLength,target.resizable,
                __completion(function(){return same.slice(0)}),
                __completion(function(){return detached.slice(0)}),
                __completion(function(){return shrunk.slice(2,6)}),
                __completion(function(){
                    var buffer=new ArrayBuffer(4);
                    buffer.constructor={
                        get [Symbol.species](){
                            return function(){return {}};
                        }
                    };
                    return buffer.slice(0);
                }),
                __completion(function(){
                    var buffer=new ArrayBuffer(4);
                    buffer.constructor={
                        get [Symbol.species](){
                            return function(){return new ArrayBuffer(1)};
                        }
                    };
                    return buffer.slice(0);
                })
            ].join("|");
        })()
// case: detached getters brand errors and toStringTag fallback
(function(){
            var source=new ArrayBuffer(4,{maxByteLength:8});
            var moved=source.transfer();
            var tagged=new ArrayBuffer(0);
            var before=Object.prototype.toString.call(tagged);
            delete ArrayBuffer.prototype[Symbol.toStringTag];
            return [
                source.byteLength,source.maxByteLength,source.resizable,
                source.detached,moved.byteLength,moved.maxByteLength,
                moved.resizable,moved.detached,
                __completion(function(){return source.resize(1)}),
                __completion(function(){return source.slice(0)}),
                __completion(function(){return source.transfer()}),
                before,Object.prototype.toString.call(tagged),
                __completion(function(){
                    return Object.getOwnPropertyDescriptor(
                        ArrayBuffer.prototype,"byteLength").get.call({});
                })
            ].join("|");
        })()
