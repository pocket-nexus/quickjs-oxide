// case: brand and initial buffer-state validation
(function(){
            function completion(thunk){
                try{return "return:"+String(thunk())}
                catch(error){return "throw:"+error.name+":"+error.message}
            }
            var base=Object.getPrototypeOf(Uint8Array.prototype);
            var rab=new ArrayBuffer(4,{maxByteLength:8});
            var oob=new Uint8Array(rab,0,4);
            rab.resize(2);
            var detachedBuffer=new ArrayBuffer(4);
            var detached=new Uint8Array(detachedBuffer);
            detachedBuffer.transfer();
            return [
                completion(function(){return base.entries.call({})}),
                completion(function(){
                    return base.keys.call(new Proxy(oob,{}));
                }),
                completion(function(){return base.entries.call(oob)}),
                completion(function(){return base.keys.call(oob)}),
                completion(function(){return base.values.call(oob)}),
                completion(function(){return base[Symbol.iterator].call(oob)}),
                completion(function(){return base.entries.call(detached)}),
                base[Symbol.iterator]===base.values,
                (function(){
                    var savedAlias=base[Symbol.iterator];
                    var originalValues=base.values;
                    base.values=function(){return "replacement"};
                    var unchangedAfterReplace=
                        base[Symbol.iterator]===savedAlias &&
                        savedAlias===originalValues;
                    delete base.values;
                    var result=savedAlias.call(
                        new Uint8Array([13])
                    ).next();
                    return unchangedAfterReplace+":"+
                        (base[Symbol.iterator]===savedAlias)+":"+
                        result.value+":"+result.done;
                })()
            ].join("|");
        })()
// case: live RAB growth shrink detach and terminal state
(function(){
            function step(result){
                return result.done ? "done" : "value:"+String(result.value);
            }
            function entry(result){
                return result.done
                    ? "done"
                    : "entry:"+result.value[0]+","+String(result.value[1]);
            }
            function completion(thunk){
                try{return "return:"+step(thunk())}
                catch(error){return "throw:"+error.name+":"+error.message}
            }
            var out=[];

            var buffer=new ArrayBuffer(2,{maxByteLength:4});
            var tracking=new Uint8Array(buffer);
            tracking.set([1,2]);
            var entries=tracking.entries();
            out.push(entry(entries.next()));
            buffer.resize(4);
            tracking[2]=3;
            tracking[3]=4;
            out.push(
                entry(entries.next()),
                entry(entries.next()),
                entry(entries.next()),
                entry(entries.next())
            );

            buffer=new ArrayBuffer(4,{maxByteLength:8});
            tracking=new Uint8Array(buffer);
            var keys=tracking.keys();
            out.push(step(keys.next()),step(keys.next()));
            buffer.resize(1);
            out.push(step(keys.next()));
            buffer.resize(6);
            out.push(step(keys.next()));

            buffer=new ArrayBuffer(4,{maxByteLength:8});
            var fixed=new Uint8Array(buffer,0,4);
            fixed.set([5,6,7,8]);
            entries=fixed.entries();
            out.push(entry(entries.next()));
            buffer.resize(2);
            out.push(completion(function(){return entries.next()}));
            buffer.resize(4);
            out.push(
                entry(entries.next()),
                entry(entries.next()),
                entry(entries.next()),
                entry(entries.next())
            );

            buffer=new ArrayBuffer(4);
            tracking=new Uint8Array(buffer);
            var values=tracking.values();
            out.push(step(values.next()));
            buffer.transfer();
            out.push(
                completion(function(){return values.next()}),
                completion(function(){return values.next()})
            );

            buffer=new ArrayBuffer(1,{maxByteLength:4});
            tracking=new Uint8Array(buffer);
            keys=tracking.keys();
            out.push(step(keys.next()),step(keys.next()));
            buffer.resize(4);
            out.push(step(keys.next()));
            buffer.transfer();
            out.push(step(keys.next()));

            buffer=new ArrayBuffer(5,{maxByteLength:9});
            var words=new Uint16Array(buffer);
            words[0]=257;
            words[1]=514;
            entries=words.entries();
            out.push("length:"+words.length,entry(entries.next()));
            buffer.resize(7);
            out.push(
                "length:"+words.length,
                entry(entries.next()),
                entry(entries.next()),
                entry(entries.next())
            );

            return out.join("|");
        })()
// case: all concrete TypedArray classes share the iterator kernel
(function(){
            var constructors=[
                Int8Array,Uint8Array,Uint8ClampedArray,
                Int16Array,Uint16Array,Int32Array,Uint32Array,
                BigInt64Array,BigUint64Array,
                Float16Array,Float32Array,Float64Array
            ];
            var arrayIteratorPrototype=Object.getPrototypeOf([].values());
            return constructors.map(function(C){
                var bigint=C===BigInt64Array || C===BigUint64Array;
                var value=new C(bigint ? [7n,9n] : [7,9]);
                var pair=value.entries().next().value;
                var key=value.keys().next().value;
                var item=value.values().next().value;
                var aliasItem=value[Symbol.iterator]().next().value;
                return C.name+":"+pair[0]+","+String(pair[1])+":"+key+":"+
                    String(item)+":"+String(aliasItem)+":"+
                    (Object.getPrototypeOf(value.entries())===
                        arrayIteratorPrototype);
            }).join("|");
        })()
