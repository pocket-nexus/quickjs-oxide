// VALUE_CASES
// case: dense fill mutates and returns the receiver
(function(){
            var source=[0,1,2,3],result=source.fill(9,1,3);
            return (result===source)+"|"+source.length+"|"+source[0]+"|"+
                source[1]+"|"+source[2]+"|"+source[3];
        })()
// case: holes become own properties and omitted value is undefined
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=Array(3);source.fill();
            return source.length+"|"+own(source,0)+"|"+own(source,1)+"|"+
                own(source,2)+"|"+(source[0]===undefined)+"|"+
                (source[1]===undefined)+"|"+(source[2]===undefined);
        })()
// case: fill value is copied without coercion
(function(){
            var log="",value=Object();
            value.valueOf=function(){log+="v";throw 61};
            value.toString=function(){log+="s";throw 62};
            var source=[0,0],result=source.fill(value);
            return (result===source)+"|"+(source[0]===value)+"|"+
                (source[1]===value)+"|"+log;
        })()
// case: an inherited setter intercepts Set without creating an own property
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var log="",proto=Object(),source=Object();
            proto.__defineSetter__("1",function(value){log+="s"+value});
            Object.setPrototypeOf(source,proto);source.length=3;
            Array.prototype.fill.call(source,"x",0,3);
            return source[0]+"|"+own(source,0)+"|"+own(source,1)+"|"+
                source[2]+"|"+own(source,2)+"|"+log;
        })()
// BOUND_CASES
// case: start uses saturating Int64 conversion and negative-length offset
(function(){
            function run(start){var a=[0,1,2,3];a.fill(9,start);return ""+a[0]+a[1]+a[2]+a[3]}
            return run(undefined)+"|"+run(0/0)+"|"+run(-0)+"|"+run(1.9)+"|"+
                run(-1.9)+"|"+run(-9)+"|"+run(1/0)+"|"+run(-1/0)+"|"+run("2");
        })()
// case: end uses saturating Int64 conversion and explicit undefined means length
(function(){
            function run(end){var a=[0,1,2,3];a.fill(9,1,end);return ""+a[0]+a[1]+a[2]+a[3]}
            return run(undefined)+"|"+run(0/0)+"|"+run(2.9)+"|"+run(-1.9)+"|"+
                run(-9)+"|"+run(1/0)+"|"+run(-1/0)+"|"+run("3");
        })()
// case: zero length still converts explicit start and end
(function(){
            var log="",source=Object(),start=Object(),end=Object();
            source.__defineGetter__("length",function(){log+="L";return 0});
            start.valueOf=function(){log+="S";return 1};
            end.valueOf=function(){log+="E";return 2};
            var result=Array.prototype.fill.call(source,7,start,end);
            return (result===source)+"|"+log;
        })()
// case: MAX_SAFE_INTEGER length supports a narrow high-index range
(function(){
            var source=Object();source.length=9007199254740991;
            var result=Array.prototype.fill.call(source,7,9007199254740990);
            return (result===source)+"|"+source[9007199254740990]+"|"+
                Object.prototype.hasOwnProperty.call(source,"9007199254740990");
        })()
// ORDER_CASES
// case: length then start then end then ascending throwing Set order
(function(){
            var log="",source=Object(),length=Object(),start=Object(),end=Object();
            source.__defineGetter__("length",function(){log+="L";return length});
            length.valueOf=function(){log+="N";return 3};
            start.valueOf=function(){log+="S";return 0};
            end.valueOf=function(){log+="E";return 3};
            source.__defineSetter__("0",function(value){log+="0"+value});
            source.__defineSetter__("1",function(value){log+="1"+value});
            source.__defineSetter__("2",function(value){log+="2"+value});
            var result=Array.prototype.fill.call(source,"x",start,end);
            return (result===source)+"|"+log;
        })()
// case: a failed Set preserves earlier mutation and skips later indices
(function(){
            var source=[0,1,2],descriptor=Object();
            descriptor.value=1;descriptor.writable=false;descriptor.configurable=true;
            Object.defineProperty(source,"1",descriptor);
            try{source.fill(9);return "missing"}
            catch(error){return source[0]+"|"+source[1]+"|"+source[2]+"|"+
                error.name+"|"+error.message}
        })()
// case: a start throw prevents end conversion and indexed writes
(function(){
            var log="",source=Object(),start=Object(),end=Object();source.length=2;
            start.valueOf=function(){log+="S";throw 71};
            end.valueOf=function(){log+="E";return 2};
            source.__defineSetter__("0",function(){log+="0"});
            try{Array.prototype.fill.call(source,9,start,end);return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// case: an end throw follows start conversion but precedes indexed writes
(function(){
            var log="",source=Object(),start=Object(),end=Object();source.length=2;
            start.valueOf=function(){log+="S";return 0};
            end.valueOf=function(){log+="E";throw 72};
            source.__defineSetter__("0",function(){log+="0"});
            try{Array.prototype.fill.call(source,9,start,end);return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// GENERIC_CASES
// case: ordinary array-like receiver is mutated in place
(function(){
            var source=Object();source[0]="a";source.length=3;
            var result=Array.prototype.fill.call(source,"x",1);
            return (result===source)+"|"+source[0]+"|"+source[1]+"|"+source[2]+"|"+source.length;
        })()
// case: String indexed properties reject the first attempted Set
(function(){
            try{Array.prototype.fill.call("abc","x",1,2);return "missing"}
            catch(error){return error.name+"|"+error.message}
        })()
// case: primitive receivers with zero length return their wrappers
(function(){
            var number=Array.prototype.fill.call(7,"x");
            var boolean=Array.prototype.fill.call(true,"x");
            return Object.prototype.toString.call(number)+"|"+
                Object.prototype.toString.call(boolean)+"|"+typeof number+"|"+typeof boolean;
        })()
// ERROR_CASES
// case: null receiver
Array.prototype.fill.call(null,1)
// case: undefined receiver
Array.prototype.fill.call(undefined,1)
// case: Symbol start
[0].fill(1,Symbol('start'))
// case: Symbol end
[0].fill(1,0,Symbol('end'))
// case: BigInt start
[0].fill(1,0n)
// case: BigInt end
[0].fill(1,0,1n)
// case: Symbol length
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.fill.call(source,1)})()
