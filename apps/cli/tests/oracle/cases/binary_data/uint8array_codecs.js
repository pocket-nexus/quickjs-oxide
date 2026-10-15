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
    catch(error){
        if(error===__token)return "throw:token";
        return "throw:"+error.name+":"+error.message;
    }
}
function __bytes(value){return Array.prototype.join.call(value,",")}
function __result(value){return value.read+","+value.written}
function __metadata(owner,key){
    var fn=owner[key];
    return key+":"+fn.name+":"+fn.length+":"+__isConstructor(fn)+":"+
        __bits(owner,key)+":"+__bits(fn,"name")+":"+__bits(fn,"length")+":"+
        Reflect.ownKeys(fn).map(String).join(",");
}
var __token={};
// case: surface placement order descriptors arity and constructability
(function(){
            return [
                "staticKeys="+Reflect.ownKeys(Uint8Array).map(String).join(","),
                "protoKeys="+Reflect.ownKeys(Uint8Array.prototype).
                    map(String).join(","),
                "static="+["fromBase64","fromHex"].map(function(key){
                    return __metadata(Uint8Array,key);
                }).join(";"),
                "proto="+[
                    "toBase64","toHex","setFromBase64","setFromHex"
                ].map(function(key){
                    return __metadata(Uint8Array.prototype,key);
                }).join(";")
            ].join("|");
        })()
// case: base64 alphabets padding whitespace hex case and result prototypes
(function(){
            var one=new Uint8Array([102]);
            var value=new Uint8Array([0,1,2,253,254,255]);
            var base64=Uint8Array.fromBase64(" \tAAEC\n/f7/\r ");
            var base64url=Uint8Array.fromBase64(
                "AAEC_f7_",{alphabet:"base64url"});
            var hex=Uint8Array.fromHex("000102FdFeFf");
            return [
                one.toBase64(),one.toBase64({omitPadding:true}),
                value.toBase64(),
                value.toBase64({alphabet:"base64url"}),
                value.toHex(),
                __bytes(base64),__bytes(base64url),__bytes(hex),
                __bytes(Uint8Array.fromBase64("")),
                __bytes(Uint8Array.fromHex("")),
                Object.getPrototypeOf(base64)===Uint8Array.prototype,
                Object.getPrototypeOf(hex)===Uint8Array.prototype
            ].join("|");
        })()
// case: base64 loose strict and stop-before-partial final chunks
(function(){
            function decode(input,options){
                return __completion(function(){
                    return __bytes(Uint8Array.fromBase64(input,options));
                });
            }
            return [
                decode("Zg"),
                decode("Zg",{lastChunkHandling:"strict"}),
                decode("Zg==",{lastChunkHandling:"strict"}),
                decode("Zg",{lastChunkHandling:"stop-before-partial"}),
                decode("Zm8"),
                decode("Zm8",{lastChunkHandling:"strict"}),
                decode("Zm8=",{lastChunkHandling:"strict"}),
                decode("Zm8",{lastChunkHandling:"stop-before-partial"}),
                decode("Zh==",{lastChunkHandling:"strict"}),
                decode("Zh=="),
                decode("Zm9",{lastChunkHandling:"strict"}),
                decode("Zm9"),
                decode("Zm9vYg==garbage"),
                decode("Zm9vYg==",{
                    lastChunkHandling:"stop-before-partial"
                }),
                decode(" Z m 9 v \n Y g == ",{
                    lastChunkHandling:"strict"
                }),
                decode("Z\u000bg=="),
                decode("Z\u00a0g=="),
                decode("Z\u000cg==")
            ].join("|");
        })()
// case: option getter order mutation and validation short circuits
(function(){
            var log="",value=new Uint8Array([0]),toOptions={};
            Object.defineProperty(toOptions,"alphabet",{
                get:function(){
                    log+="A";value[0]=255;return "base64";
                }
            });
            Object.defineProperty(toOptions,"omitPadding",{
                get:function(){log+="O";return true}
            });
            var encoded=value.toBase64(toOptions);

            var fromOptions={};
            Object.defineProperty(fromOptions,"alphabet",{
                get:function(){log+="B";return "base64"}
            });
            Object.defineProperty(fromOptions,"lastChunkHandling",{
                get:function(){log+="L";return "loose"}
            });
            var decoded=Uint8Array.fromBase64("Zg==",fromOptions);

            var target=new Uint8Array(1),setOptions={};
            Object.defineProperty(setOptions,"alphabet",{
                get:function(){log+="C";return "base64"}
            });
            Object.defineProperty(setOptions,"lastChunkHandling",{
                get:function(){log+="H";return "strict"}
            });
            var setResult=target.setFromBase64("Zg==",setOptions);

            var evil={
                toString:function(){log+="T";throw __token}
            };
            var touch={};
            Object.defineProperty(touch,"alphabet",{
                get:function(){log+="X";throw __token}
            });
            var inputCheck=__completion(function(){
                return Uint8Array.fromBase64(evil,touch);
            });
            var setInputCheck=__completion(function(){
                return target.setFromBase64(evil,touch);
            });
            var brandCheck=__completion(function(){
                return Uint8Array.prototype.toBase64.call([],touch);
            });
            var setBrandCheck=__completion(function(){
                return Uint8Array.prototype.setFromBase64.call(
                    [],evil,touch);
            });
            var enumCheck=__completion(function(){
                return value.toBase64({alphabet:evil});
            });
            var invalidOrder={};
            Object.defineProperty(invalidOrder,"alphabet",{
                get:function(){log+="I";return "invalid"}
            });
            Object.defineProperty(invalidOrder,"lastChunkHandling",{
                get:function(){log+="J";throw __token}
            });
            var invalidOrderCheck=__completion(function(){
                return Uint8Array.fromBase64("Zg==",invalidOrder);
            });
            return [
                log,encoded,__bytes(decoded),__result(setResult),
                __bytes(target),inputCheck,setInputCheck,brandCheck,
                setBrandCheck,enumCheck,invalidOrderCheck
            ].join("|");
        })()
// case: invalid input option brand and constructor errors
(function(){
            var calls=[
                function(){return Uint8Array.fromBase64()},
                function(){return Uint8Array.fromBase64(1)},
                function(){return Uint8Array.fromBase64(Symbol("x"))},
                function(){return Uint8Array.fromBase64("AA.A")},
                function(){return Uint8Array.fromBase64("Zg===")},
                function(){
                    return Uint8Array.fromBase64(
                        "Zg",{lastChunkHandling:"strict"});
                },
                function(){return Uint8Array.fromBase64("Zg==",null)},
                function(){
                    return Uint8Array.fromBase64(
                        "Zg==",{alphabet:"other"});
                },
                function(){
                    return Uint8Array.fromBase64(
                        "Zg==",{lastChunkHandling:"other"});
                },
                function(){return Uint8Array.fromHex()},
                function(){return Uint8Array.fromHex({})},
                function(){return Uint8Array.fromHex("abc")},
                function(){return Uint8Array.fromHex("0g")},
                function(){return Uint8Array.fromHex("aa aa")},
                function(){
                    return Uint8Array.prototype.toBase64.call([]);
                },
                function(){
                    return Uint8Array.prototype.toHex.call(
                        new Uint8ClampedArray(1));
                },
                function(){
                    return Uint8Array.prototype.setFromBase64.call(
                        new Int8Array(1),"AA==");
                },
                function(){
                    return Uint8Array.prototype.setFromHex.call({},"00");
                },
                function(){return new Uint8Array.fromBase64("AA==")},
                function(){return new Uint8Array.prototype.toHex()}
            ];
            return calls.map(__completion).join("|");
        })()
// case: static receiver is ignored while subclass instances retain the brand
(function(){
            var log="";
            class Subclass extends Uint8Array {
                constructor(value){log+="C";super(value)}
            }
            var trapReceiver=new Proxy(function(){},{
                get:function(){log+="G";throw __token}
            });
            var a=Uint8Array.fromBase64.call(trapReceiver,"AQI=");
            var b=Uint8Array.fromHex.call(null,"0304");
            var c=Subclass.fromBase64("BQY=");
            var d=Subclass.fromHex("0708");
            var subclass=new Subclass([9,10]);
            var proxy=new Proxy(new Uint8Array([11]),{});
            return [
                log,
                Object.getPrototypeOf(a)===Uint8Array.prototype,
                Object.getPrototypeOf(b)===Uint8Array.prototype,
                Object.getPrototypeOf(c)===Uint8Array.prototype,
                Object.getPrototypeOf(d)===Uint8Array.prototype,
                __bytes(a),__bytes(b),__bytes(c),__bytes(d),
                subclass.toBase64(),subclass.toHex(),
                __completion(function(){
                    return Uint8Array.prototype.toHex.call(proxy);
                }),
                __completion(function(){
                    return Uint8Array.prototype.setFromHex.call(proxy,"0b");
                })
            ].join("|");
        })()
// case: setFromBase64 capacity subarray records and partial writes
(function(){
            function run(size,input,options){
                var target=new Uint8Array(size);
                target.fill(255);
                var result=target.setFromBase64(input,options);
                return __result(result)+":"+__bytes(target)+":"+
                    Reflect.ownKeys(result).join(",")+":"+
                    (Object.getPrototypeOf(result)===Object.prototype);
            }
            var base=new Uint8Array([255,255,255,255,255,255,255]);
            var subarray=base.subarray(2,5);
            var subResult=subarray.setFromBase64("Zm9vYmFy");

            var partial=new Uint8Array(6);
            partial.fill(255);
            var partialError=__completion(function(){
                return partial.setFromBase64("MjYyZm.9v");
            });

            var strict=new Uint8Array(6);
            strict.fill(255);
            var strictError=__completion(function(){
                return strict.setFromBase64(
                    "MjYyZg",{lastChunkHandling:"strict"});
            });
            return [
                run(5,"Zm9vYmFy"),
                run(0,"Zm9v"),
                run(0,"#"),
                run(1,"Zg=="),
                run(2,"Zm9v"),
                run(3," Zm9v "),
                run(3,"Zm9v#"),
                __result(subResult)+":"+__bytes(base),
                partialError+":"+__bytes(partial),
                strictError+":"+__bytes(strict)
            ].join("|");
        })()
// case: setFromHex capacity subarray records and error atomicity
(function(){
            function run(size,input){
                var target=new Uint8Array(size);
                target.fill(255);
                var result=target.setFromHex(input);
                return __result(result)+":"+__bytes(target)+":"+
                    Reflect.ownKeys(result).join(",")+":"+
                    (Object.getPrototypeOf(result)===Object.prototype);
            }
            function fail(input){
                var target=new Uint8Array(5);
                target.fill(255);
                return __completion(function(){
                    return target.setFromHex(input);
                })+":"+__bytes(target);
            }
            var base=new Uint8Array([255,255,255,255,255,255,255]);
            var subarray=base.subarray(2,5);
            var subResult=subarray.setFromHex("aabbccdd");
            return [
                run(2,"aabbcc"),
                run(0,"aabb"),
                run(0,"G0"),
                run(3,"AaBbCc"),
                run(2,"aabbGG"),
                __result(subResult)+":"+__bytes(base),
                fail("aabbg0"),
                fail("aabbc"),
                fail("aaa "),
                (function(){
                    var empty=new Uint8Array(0);
                    return __completion(function(){
                        return empty.setFromHex("1");
                    })+":"+__bytes(empty);
                })()
            ].join("|");
        })()
// case: detached and out-of-bounds views revalidate after base64 options
(function(){
            var log="";
            var a=new Uint8Array([1]);
            var aOptions={
                get alphabet(){
                    log+="A";a.buffer.transfer();return "base64";
                },
                get omitPadding(){log+="O";return false}
            };
            var aResult=__completion(function(){
                return a.toBase64(aOptions);
            });

            var b=new Uint8Array([2]);
            b.buffer.transfer();
            var bOptions={
                get alphabet(){log+="B";return "base64"},
                get omitPadding(){log+="P";return false}
            };
            var bResult=__completion(function(){
                return b.toBase64(bOptions);
            });

            var c=new Uint8Array([3]);
            var cOptions={
                get alphabet(){
                    log+="C";c.buffer.transfer();return "base64";
                },
                get lastChunkHandling(){log+="H";return "loose"}
            };
            var cResult=__completion(function(){
                return c.setFromBase64("Aw==",cOptions);
            });

            var d=new Uint8Array([4]);
            d.buffer.transfer();
            var dResult=__completion(function(){return d.toHex()});
            var eResult=__completion(function(){
                return d.setFromHex("04");
            });

            var rab=new ArrayBuffer(4,{maxByteLength:8});
            var fixed=new Uint8Array(rab,2,2);
            rab.resize(1);
            var fixedOptions={
                get alphabet(){log+="F";return "base64"},
                get omitPadding(){log+="Q";return false}
            };
            var fResult=__completion(function(){
                return fixed.toBase64(fixedOptions);
            });
            var gResult=__completion(function(){return fixed.toHex()});
            var hResult=__completion(function(){
                return fixed.setFromBase64("AA==");
            });
            var iResult=__completion(function(){
                return fixed.setFromHex("00");
            });

            var growable=new ArrayBuffer(4,{maxByteLength:8});
            var tracking=new Uint8Array(growable,2);
            growable.resize(1);
            var jResult=__completion(function(){
                return tracking.toBase64();
            });
            var kResult=__completion(function(){return tracking.toHex()});

            var changingBuffer=new ArrayBuffer(4,{maxByteLength:8});
            var changing=new Uint8Array(changingBuffer);
            var changingOptions={
                get alphabet(){
                    log+="G";changingBuffer.resize(2);return "base64";
                },
                get omitPadding(){log+="R";return true}
            };
            var lResult=__completion(function(){
                return changing.toBase64(changingOptions);
            });
            return [
                log,aResult,bResult,cResult,dResult,eResult,
                fResult,gResult,hResult,iResult,jResult,kResult,
                lResult,changing.length
            ].join("|");
        })()
// case: lone surrogates and non-BMP input reject without string coercion
(function(){
            function fromBase64(input){
                return __completion(function(){
                    return __bytes(Uint8Array.fromBase64(input));
                });
            }
            function fromHex(input){
                return __completion(function(){
                    return __bytes(Uint8Array.fromHex(input));
                });
            }
            function setBase64(input,size){
                if(size===undefined)size=5;
                var target=new Uint8Array(size);
                target.fill(255);
                return __completion(function(){
                    return __result(target.setFromBase64(input));
                })+":"+__bytes(target);
            }
            function setHex(input,size){
                if(size===undefined)size=5;
                var target=new Uint8Array(size);
                target.fill(255);
                return __completion(function(){
                    return __result(target.setFromHex(input));
                })+":"+__bytes(target);
            }
            var boxed=new String("Zg==");
            boxed.toString=function(){throw __token};
            return [
                fromBase64("Zm9v\uD800"),
                fromBase64("\uDC00"),
                fromBase64("Zm9v😀"),
                fromHex("aa\uD800"),
                fromHex("aabb\uD800g"),
                setBase64("Zm9v\uD800"),
                setHex("aabb\uD800g"),
                setHex("aaé"),
                setHex("aa\uD800"),
                setHex("aa😀"),
                setBase64("AAAA\uD800",3),
                setBase64("AAAA\uD800",4),
                setBase64("#",0),
                setHex("G0",0),
                setHex("1",0),
                __completion(function(){
                    return Uint8Array.fromBase64(boxed);
                }),
                __completion(function(){
                    return new Uint8Array([1]).toBase64({
                        alphabet:"\uD800"
                    });
                })
            ].join("|");
        })()
