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
function __keys(object){return Reflect.ownKeys(object).map(String).join(",")}
function __metadata(owner,key){
    var fn=owner[key];
    return String(key)+":"+fn.name+":"+fn.length+":"+__isConstructor(fn)+":"+
        __bits(owner,key)+":"+__keys(fn);
}
function __accessor(owner,key){
    var descriptor=Object.getOwnPropertyDescriptor(owner,key);
    var getter=descriptor.get;
    return String(key)+":"+getter.name+":"+getter.length+":"+
        __isConstructor(getter)+":"+__bits(owner,key)+":"+__keys(getter);
}
function __norm(value){
    if(typeof value==="bigint")return String(value)+"n";
    if(typeof value==="number"){
        if(value!==value)return "NaN";
        if(Object.is(value,-0))return "-0";
        if(value===Infinity)return "Infinity";
        if(value===-Infinity)return "-Infinity";
    }
    return String(value);
}
function __outcome(thunk){
    try{return "return:"+__norm(thunk())}
    catch(error){return "throw:"+error.name}
}
function __hex(view,start,length){
    var result="";
    for(var index=0;index<length;index++){
        var byte=view.getUint8(start+index).toString(16);
        result+=(byte.length===1?"0":"")+byte;
    }
    return result;
}
// case: constructor prototype descriptors and complete method graph
(function(){
            var getters=["buffer","byteLength","byteOffset"];
            var suffixes=[
                "Int8","Uint8","Int16","Uint16","Int32","Uint32",
                "BigInt64","BigUint64","Float16","Float32","Float64"
            ];
            var methods=[];
            suffixes.forEach(function(suffix){
                methods.push(__metadata(DataView.prototype,"get"+suffix));
            });
            suffixes.forEach(function(suffix){
                methods.push(__metadata(DataView.prototype,"set"+suffix));
            });
            var view=new DataView(new ArrayBuffer(1));
            return [
                "global="+__bits(globalThis,"DataView"),
                "ctor="+DataView.name+":"+DataView.length+":"+
                    __isConstructor(DataView)+":"+__keys(DataView),
                "links="+(Object.getPrototypeOf(DataView)===Function.prototype)+":"+
                    (Object.getPrototypeOf(DataView.prototype)===Object.prototype)+":"+
                    (DataView.prototype.constructor===DataView),
                "proto="+__keys(DataView.prototype),
                "accessors="+getters.map(function(key){
                    return __accessor(DataView.prototype,key);
                }).join(";"),
                "methods="+methods.join(";"),
                "tag="+__bits(DataView.prototype,Symbol.toStringTag)+":"+
                    DataView.prototype[Symbol.toStringTag],
                "brand="+ArrayBuffer.isView(view)+":"+
                    ArrayBuffer.isView(Object.create(DataView.prototype))+":"+
                    ArrayBuffer.isView(new Proxy(view,{}))
            ].join("|");
        })()
// case: all eleven element formats preserve numeric boundaries and endian bytes
(function(){
            var specs=[
                ["Int8",1,255],
                ["Uint8",1,-1],
                ["Int16",2,32769],
                ["Uint16",2,-2],
                ["Int32",4,2147483649],
                ["Uint32",4,-1],
                ["BigInt64",8,9223372036854775809n],
                ["BigUint64",8,-1n],
                ["Float16",2,65504],
                ["Float32",4,3.402823669209385e38],
                ["Float64",8,-0]
            ];
            var observations=specs.map(function(spec){
                var suffix=spec[0],width=spec[1],value=spec[2];
                var view=new DataView(new ArrayBuffer(width*2));
                view["set"+suffix](0,value,false);
                var big=__hex(view,0,width)+":"+
                    __norm(view["get"+suffix](0,false));
                view["set"+suffix](width,value,true);
                var little=__hex(view,width,width)+":"+
                    __norm(view["get"+suffix](width,true));
                return suffix+"="+big+":"+little;
            });
            var edges=new DataView(new ArrayBuffer(8));
            edges.setInt8(0,Infinity);
            edges.setFloat16(2,NaN);
            return observations.join("|")+"|edges="+
                __norm(edges.getInt8(0))+":"+
                __norm(edges.getFloat16(2))+":"+
                __outcome(function(){return edges.setBigInt64(0,1)} )+":"+
                __outcome(function(){return edges.setInt32(0,1n)});
        })()
// case: fixed and tracking views become out of bounds and recover across RAB resize
(function(){
            function state(label,view,buffer){
                return label+":"+
                    __outcome(function(){return view.byteOffset})+":"+
                    __outcome(function(){return view.byteLength})+":"+
                    (view.buffer===buffer);
            }
            var buffer=new ArrayBuffer(8,{maxByteLength:16});
            var fixed=new DataView(buffer,2,4);
            var tracking=new DataView(buffer,2);
            fixed.setUint16(0,0x1234);
            tracking.setUint8(5,0x7f);
            var result=[
                state("initial-fixed",fixed,buffer),
                state("initial-tracking",tracking,buffer)
            ];
            buffer.resize(4);
            result.push(state("shrink4-fixed",fixed,buffer));
            result.push(state("shrink4-tracking",tracking,buffer));
            result.push("shrink4-read="+
                __outcome(function(){return fixed.getUint8(0)})+":"+
                __outcome(function(){return tracking.getUint16(0)}));
            buffer.resize(8);
            result.push(state("grow8-fixed",fixed,buffer));
            result.push(state("grow8-tracking",tracking,buffer));
            result.push("grow8-read="+fixed.getUint16(0)+":"+
                tracking.getUint8(5));
            buffer.resize(2);
            result.push(state("equal-offset",tracking,buffer));
            result.push("equal-offset-read="+
                __outcome(function(){return tracking.getUint8(0)}));
            buffer.resize(1);
            result.push(state("shrink1-fixed",fixed,buffer));
            result.push(state("shrink1-tracking",tracking,buffer));
            buffer.resize(10);
            result.push(state("grow10-fixed",fixed,buffer));
            result.push(state("grow10-tracking",tracking,buffer));
            result.push("grow10-read="+fixed.getUint16(0)+":"+
                tracking.getUint8(5));
            return result.join("|");
        })()
// case: detach preserves the DataView brand and buffer edge while invalidating access
(function(){
            var buffer=new ArrayBuffer(8,{maxByteLength:16});
            var fixed=new DataView(buffer,2,4);
            var tracking=new DataView(buffer,2);
            fixed.setUint32(0,0x12345678);
            var moved=buffer.transfer();
            var bufferGetter=Object.getOwnPropertyDescriptor(
                DataView.prototype,"buffer").get;
            return [
                "source="+buffer.byteLength+":"+buffer.maxByteLength+":"+
                    buffer.resizable+":"+buffer.detached,
                "moved="+moved.byteLength+":"+moved.maxByteLength+":"+
                    moved.resizable+":"+moved.detached,
                "edges="+(fixed.buffer===buffer)+":"+
                    (tracking.buffer===buffer),
                "fixed="+__outcome(function(){return fixed.byteOffset})+":"+
                    __outcome(function(){return fixed.byteLength})+":"+
                    __outcome(function(){return fixed.getUint32(0)})+":"+
                    __outcome(function(){return fixed.setUint32(0,1)}),
                "tracking="+__outcome(function(){return tracking.byteOffset})+":"+
                    __outcome(function(){return tracking.byteLength})+":"+
                    __outcome(function(){return tracking.getUint8(0)}),
                "brand="+ArrayBuffer.isView(fixed)+":"+
                    ArrayBuffer.isView(tracking)+":"+
                    Object.prototype.toString.call(fixed),
                "wrong="+__outcome(function(){return bufferGetter.call({})})+":"+
                    __outcome(function(){
                        return DataView.prototype.getUint8.call({},0);
                    })
            ].join("|");
        })()
// case: constructor observes buffer and coercions before newTarget then revalidates
(function(){
            var result=[];
            (function(){
                var log="",buffer=new ArrayBuffer(8,{maxByteLength:16});
                var custom=Object.create(DataView.prototype);
                var NewTarget=(function(){}).bind(null);
                Object.defineProperty(NewTarget,"prototype",{
                    get:function(){log+="p";return custom}
                });
                var offset={valueOf:function(){log+="o";return 2}};
                var length={valueOf:function(){log+="l";return 3}};
                var view=Reflect.construct(
                    DataView,[buffer,offset,length],NewTarget);
                result.push("success="+log+":"+
                    (Object.getPrototypeOf(view)===custom)+":"+
                    view.byteOffset+":"+view.byteLength);
            })();
            (function(){
                var log="",offset={valueOf:function(){log+="o";return 0}};
                result.push("brand="+
                    __outcome(function(){return new DataView({},offset)})+
                    ":"+log);
            })();
            (function(){
                var log="",buffer=new ArrayBuffer(4);
                var NewTarget=(function(){}).bind(null);
                Object.defineProperty(NewTarget,"prototype",{
                    get:function(){log+="p";return DataView.prototype}
                });
                var offset={valueOf:function(){
                    log+="o";throw new RangeError("offset");
                }};
                var length={valueOf:function(){log+="l";return 1}};
                result.push("offset-throw="+__outcome(function(){
                    return Reflect.construct(
                        DataView,[buffer,offset,length],NewTarget);
                })+":"+log);
            })();
            (function(){
                var log="",buffer=new ArrayBuffer(4);
                var NewTarget=(function(){}).bind(null);
                Object.defineProperty(NewTarget,"prototype",{
                    get:function(){log+="p";return DataView.prototype}
                });
                var offset={valueOf:function(){
                    log+="o";buffer.transfer();return 0;
                }};
                var length={valueOf:function(){log+="l";return 1}};
                result.push("offset-detach="+__outcome(function(){
                    return Reflect.construct(
                        DataView,[buffer,offset,length],NewTarget);
                })+":"+log);
            })();
            (function(){
                var log="",buffer=new ArrayBuffer(4);
                var NewTarget=(function(){}).bind(null);
                Object.defineProperty(NewTarget,"prototype",{
                    get:function(){log+="p";return DataView.prototype}
                });
                var length={valueOf:function(){
                    log+="l";buffer.transfer();return 1;
                }};
                result.push("length-detach="+__outcome(function(){
                    return Reflect.construct(
                        DataView,[buffer,0,length],NewTarget);
                })+":"+log);
            })();
            (function(){
                var log="",buffer=new ArrayBuffer(8,{maxByteLength:8});
                var NewTarget=(function(){}).bind(null);
                Object.defineProperty(NewTarget,"prototype",{
                    get:function(){
                        log+="p";buffer.resize(1);return DataView.prototype;
                    }
                });
                var offset={valueOf:function(){log+="o";return 2}};
                var length={valueOf:function(){log+="l";return 3}};
                result.push("prototype-shrink="+__outcome(function(){
                    return Reflect.construct(
                        DataView,[buffer,offset,length],NewTarget);
                })+":"+log);
            })();
            (function(){
                var rab=new ArrayBuffer(8,{maxByteLength:16});
                var fixed=new ArrayBuffer(8);
                result.push("omitted="+
                    new DataView(rab,2).byteLength+":"+
                    new DataView(rab,2,undefined).byteLength+":"+
                    new DataView(fixed,2).byteLength);
            })();
            result.push("call="+
                __outcome(function(){return DataView(new ArrayBuffer(0))}));
            result.push("ranges="+
                __outcome(function(){
                    return new DataView(new ArrayBuffer(4),-1);
                })+":"+
                __outcome(function(){
                    return new DataView(new ArrayBuffer(4),2,3);
                }));
            return result.join("|");
        })()
