// mode: completion-array
// case: classic unary functions preserve their pinned boundaries
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.abs(-3.5)),show(Math.abs(-0)),show(Math.floor(-1.25)),
                show(Math.ceil(-1.25)),show(Math.round(-1.5)),show(Math.sqrt(9)),
                show(Math.sqrt(-1)),show(Math.acos(1)),show(Math.asin(0)),
                show(Math.atan(Infinity)),show(Math.atan(-Infinity)),
                show(Math.cos(0)),show(Math.exp(0)),show(Math.log(1)),
                show(Math.sin(-0)),show(Math.tan(-0))
            ];
        })()
// case: ES2015 unary functions preserve signed zero NaN and infinity
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.trunc(-1.9)),show(Math.trunc(-0.1)),
                show(Math.sign(-4)),show(Math.sign(-0)),show(Math.sign(NaN)),
                show(Math.cosh(0)),show(Math.sinh(-0)),show(Math.tanh(Infinity)),
                show(Math.tanh(-Infinity)),show(Math.acosh(1)),show(Math.acosh(0)),
                show(Math.asinh(-0)),show(Math.atanh(-0)),show(Math.atanh(1)),
                show(Math.expm1(-0)),show(Math.log1p(-0)),show(Math.log1p(-2)),
                show(Math.log2(8)),show(Math.log10(1000)),show(Math.cbrt(-8)),
                show(Math.cbrt(-0))
            ];
        })()
// case: atanh keeps pinned accuracy across its fdlibm branches
(function(){
            var floats=new Float64Array(2),words=new Uint32Array(floats.buffer),little=true;
            floats[0]=2;floats[1]=4;
            if(Math.abs((words[3]-words[1])*4294967296+words[2]-words[0])===1048576)
                little=false;
            function distance(left,right){
                floats[0]=left;floats[1]=right;
                var leftHigh=words[little?1:0],leftLow=words[little?0:1];
                var rightHigh=words[little?3:2],rightLow=words[little?2:3];
                return Math.abs((rightHigh-leftHigh)*4294967296+rightLow-leftLow);
            }
            var vectors=[
                [-0.9999983310699463,-6.998237084679027],
                [-0.999992847442627,-6.2705920974657525],
                [-0.9999828338623047,-5.832855225378502],
                [-0.9284839630126953,-1.6472838718760747],
                [-0.5,-0.5493061443340548],[-0.3,-0.3095196042031117],
                [0.00001,0.000010000000000333334],
                [0.3,0.3095196042031117],[0.5,0.5493061443340548],
                [0.9928233623504639,2.8132383539094192]
            ];
            var result=[];
            for(var index=0;index<vectors.length;index++)
                result.push(distance(Math.atanh(vectors[index][0]),vectors[index][1])<=2);
            result.push(1/Math.atanh(-0)===-Infinity);
            result.push(1/Math.atanh(0)===Infinity);
            result.push(Math.atanh(-1)===-Infinity);
            result.push(Math.atanh(1)===Infinity);
            result.push(Math.atanh(1.0000000000000002)!==Math.atanh(1.0000000000000002));
            result.push(Math.atanh(Infinity)!==Math.atanh(Infinity));
            return result;
        })()
// case: acosh keeps portable accuracy across its fdlibm branches
(function(){
            var floats=new Float64Array(2),words=new Uint32Array(floats.buffer),little=true;
            floats[0]=2;floats[1]=4;
            if(Math.abs((words[3]-words[1])*4294967296+words[2]-words[0])===1048576)
                little=false;
            function distance(left,right){
                floats[0]=left;floats[1]=right;
                var leftHigh=words[little?1:0],leftLow=words[little?0:1];
                var rightHigh=words[little?3:2],rightLow=words[little?2:3];
                return Math.abs((rightHigh-leftHigh)*4294967296+rightLow-leftLow);
            }
            var vectors=[
                [1.0000014305114746,0.0016914556651292944],
                [1.0000000001,0.000014142136208675862],
                [2,1.3169578969248166],
                [498.234130859375,6.9042162822876465],
                [1e300,691.4686750787737]
            ];
            var result=[];
            for(var index=0;index<vectors.length;index++)
                result.push(distance(Math.acosh(vectors[index][0]),vectors[index][1])<=9);
            result.push(1/Math.acosh(1)===Infinity);
            result.push(Math.acosh(0)!==Math.acosh(0));
            result.push(Math.acosh(Infinity)===Infinity);
            return result;
        })()
// case: round implements ties toward positive infinity and large-number edges
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.round(-0)),show(Math.round(-0.1)),show(Math.round(-0.5)),
                show(Math.round(-0.5000000000000001)),show(Math.round(0.49999999999999994)),
                show(Math.round(0.5)),show(Math.round(1.5)),show(Math.round(-1.5)),
                show(Math.round(4503599627370495.5)),
                show(Math.round(-4503599627370495.5)),
                show(Math.round(NaN)),show(Math.round(Infinity)),show(Math.round(-Infinity))
            ];
        })()
// case: atan2 quadrants and signed zeros match QuickJS libm dispatch
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.atan2(0,0)),show(Math.atan2(-0,0)),
                show(Math.atan2(0,-0)),show(Math.atan2(-0,-0)),
                show(Math.atan2(Infinity,Infinity)),show(Math.atan2(-Infinity,Infinity)),
                show(Math.atan2(Infinity,-Infinity)),show(Math.atan2(-Infinity,-Infinity)),
                show(Math.atan2(NaN,1)),show(Math.atan2(1,NaN))
            ];
        })()
// case: pow shares QuickJS unit infinity and signed-zero rules
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.pow(1,Infinity)),show(Math.pow(-1,Infinity)),
                show(Math.pow(1,-Infinity)),show(Math.pow(-1,-Infinity)),
                show(Math.pow(-0,3)),show(Math.pow(-0,2)),show(Math.pow(-0,-3)),
                show(Math.pow(-0,-2)),show(Math.pow(-2,0.5)),show(Math.pow(NaN,0)),
                show(Math.pow(2,NaN)),show(Math.pow(Infinity,-1)),
                show(Math.pow(-Infinity,-3)),show(Math.pow(-Infinity,2)),
                show(Math.pow(2,10))
            ];
        })()
// case: min and max preserve empty identity NaN and signed zero
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.min()),show(Math.max()),show(Math.min(3,-2,7)),
                show(Math.max(3,-2,7)),show(Math.min(0,-0)),show(Math.min(-0,0)),
                show(Math.max(0,-0)),show(Math.max(-0,0)),show(Math.min(NaN,1)),
                show(Math.max(1,NaN)),show(Math.min("4",null,true)),
                show(Math.max("4",null,true))
            ];
        })()
// case: min and max coerce every argument left to right even after NaN
(function(){
            var log="";
            function box(name,value){
                var object=Object();
                object.valueOf=function(){log+=name;return value};
                return object;
            }
            var minimum=Math.min(box("a",3),box("b",NaN),box("c",1));
            var first=log;log="";
            var maximum=Math.max(box("d",3),box("e",NaN),box("f",9));
            var second=log;log="";
            var abrupt;
            try{
                Math.min(box("g",NaN),box("h",2),{
                    valueOf:function(){log+="i";throw "stop"}
                });
            }catch(error){abrupt=error}
            return [minimum!==minimum,first,maximum!==maximum,second,abrupt,log];
        })()
// case: unary and binary numeric conversion is ordered and abrupt
(function(){
            var log="";
            function box(name,value){
                var object=Object();
                object.valueOf=function(){log+=name;return value};
                return object;
            }
            var absolute=Math.abs(box("a",-4));
            var power=Math.pow(box("b",2),box("c",5));
            var angle=Math.atan2(box("d",0),box("e",-0));
            var before=log,thrown;
            try{Math.pow(box("f",2),{valueOf:function(){log+="g";throw "boom"}})}
            catch(error){thrown=error}
            var bigint,symbol;
            try{Math.abs(1n)}catch(error){bigint=error.name+":"+error.message}
            try{Math.abs(Symbol("x"))}catch(error){symbol=error.name+":"+error.message}
            return [absolute,power,angle,before,thrown,log,bigint,symbol];
        })()
// case: f16round covers normal subnormal overflow and ties-to-even
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.f16round()),show(Math.f16round(0)),show(Math.f16round(-0)),
                show(Math.f16round(Infinity)),show(Math.f16round(-Infinity)),
                show(Math.f16round(NaN)),show(Math.f16round(65504)),
                show(Math.f16round(65520)),show(Math.f16round(0.00006103515625)),
                show(Math.f16round(5.960464477539063e-8)),
                show(Math.f16round(2.9802322387695312e-8)),
                show(Math.f16round(8.940696716308594e-8)),
                show(Math.f16round(1.00048828125)),
                show(Math.f16round(1.00146484375)),
                show(Math.f16round(-1.00048828125))
            ];
        })()
// case: fround covers signed zero infinities and binary32 ties-to-even
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.fround()),show(Math.fround(0)),show(Math.fround(-0)),
                show(Math.fround(Infinity)),show(Math.fround(-Infinity)),show(Math.fround(NaN)),
                show(Math.fround(1.0000000596046448)),
                show(Math.fround(1.0000001788139343)),show(Math.fround(3.4028236e38)),
                show(Math.fround(1.401298464324817e-45)),
                show(Math.fround(7.006492321624085e-46))
            ];
        })()
// case: hypot handles arity scaling infinity NaN and ordered conversion
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            var log="";
            function box(name,value){
                var object=Object();
                object.valueOf=function(){log+=name;return value};
                return object;
            }
            var converted=Math.hypot(box("a",3),box("b",4),box("c",12));
            return [
                show(Math.hypot()),show(Math.hypot(-0)),show(Math.hypot(3,4)),
                show(Math.hypot(3,4,12)),show(Math.hypot(NaN,Infinity)),
                show(Math.hypot(Infinity,NaN)),show(Math.hypot(NaN,2)),
                show(converted),log
            ];
        })()
// case: imul and clz32 apply ToUint32 and 32-bit wrapping
(function(){
            var log="";
            function box(name,value){
                var object=Object();
                object.valueOf=function(){log+=name;return value};
                return object;
            }
            var product=Math.imul(box("a",4294967295),box("b",5));
            var leading=Math.clz32(box("c",65536));
            return [
                Math.imul(),Math.imul(4294967295,5),Math.imul(2147483647,2),
                Math.imul(2147483648,2),Math.imul("3.9","4.1"),product,
                Math.clz32(),Math.clz32(0),Math.clz32(1),Math.clz32(-1),
                Math.clz32(65536),Math.clz32(4294967296),leading,log
            ];
        })()
// case: random stays in range uses a 52-bit grid and advances its stream
(function(){
            var first=Math.random(),previous=first,inRange=true,onGrid=true,changed=false;
            for(var i=0;i<128;i++){
                var value=Math.random();
                if(!(value>=0&&value<1))inRange=false;
                if(!Number.isInteger(value*4503599627370496))onGrid=false;
                if(value!==previous)changed=true;
                previous=value;
            }
            return [first>=0&&first<1,Number.isInteger(first*4503599627370496),
                    inRange,onGrid,changed];
        })()
// case: sumPrecise handles zero accuracy NaN and infinities
(function(){
            function show(x){
                if(x!==x)return "NaN";
                if(x===0)return 1/x===-Infinity?"-0":"+0";
                if(x===Infinity)return "+Infinity";
                if(x===-Infinity)return "-Infinity";
                return String(x);
            }
            return [
                show(Math.sumPrecise([])),show(Math.sumPrecise([0])),
                show(Math.sumPrecise([-0])),show(Math.sumPrecise([0,-0])),
                show(Math.sumPrecise([1,-1])),show(Math.sumPrecise([1e100,1,-1e100])),
                show(Math.sumPrecise([0.1,0.2,-0.3])),
                show(Math.sumPrecise([Infinity])),show(Math.sumPrecise([-Infinity])),
                show(Math.sumPrecise([Infinity,-Infinity])),show(Math.sumPrecise([NaN])),
                show(Math.sumPrecise([Infinity,NaN])),show(Math.sumPrecise([NaN,-Infinity]))
            ];
        })()
// case: sumPrecise accepts generic iterables and caches next once
(function(){
            var log="",index=0,iterator=Object(),iterable=Object();
            Object.defineProperty(iterator,"next",{
                configurable:true,
                get:function(){
                    log+="get-next,";
                    return function(){
                        log+="next,";
                        if(index<3)return {value:[10,20,12][index++],done:false};
                        return {value:99,done:true};
                    };
                }
            });
            iterator.return=function(){log+="return,";return {done:true}};
            iterable[Symbol.iterator]=function(){log+="iterator,";return iterator};
            return [Math.sumPrecise(iterable),log];
        })()
// case: sumPrecise rejects non-Numbers and closes while preserving its TypeError
(function(){
            var log="",step=0,iterator=Object(),iterable=Object(),observed;
            iterator.next=function(){
                log+="next,";
                if(step++===0)return {value:1,done:false};
                return {value:"2",done:false};
            };
            iterator.return=function(){log+="return,";throw "close boom"};
            iterable[Symbol.iterator]=function(){log+="iterator,";return iterator};
            try{Math.sumPrecise(iterable)}
            catch(error){observed=error.name+":"+error.message}
            var boxed,bigint,boolean;
            try{Math.sumPrecise([new Number(1)])}catch(error){boxed=error.name+":"+error.message}
            try{Math.sumPrecise([1n])}catch(error){bigint=error.name+":"+error.message}
            try{Math.sumPrecise([true])}catch(error){boolean=error.name+":"+error.message}
            return [observed,log,boxed,bigint,boolean];
        })()
// case: sumPrecise does not close when iterator next itself throws
(function(){
            var log="",iterator=Object(),iterable=Object(),observed;
            iterator.next=function(){log+="next,";throw "next boom"};
            iterator.return=function(){log+="return,";return {done:true}};
            iterable[Symbol.iterator]=function(){log+="iterator,";return iterator};
            try{Math.sumPrecise(iterable)}catch(error){observed=error}
            return [observed,log];
        })()
// case: sumPrecise preserves the pinned 129-term wrapping quirk
(function(){
            var values=[],x=0.000976562499999999891579782751449556599254719913005828857421875;
            for(var i=0;i<129;i++)values[i]=x;
            return [Math.sumPrecise(values),129*x];
        })()
// case: sumPrecise smoke (former Rust-only smoke)
Math.sumPrecise([20,22])
