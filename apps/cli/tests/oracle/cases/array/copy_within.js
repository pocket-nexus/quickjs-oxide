// VALUE_CASES
// case: non-overlapping copy runs forward and returns the receiver
(function(){
            var source=[0,1,2,3,4],result=source.copyWithin(0,3);
            return (result===source)+"|"+source.length+"|"+source[0]+"|"+
                source[1]+"|"+source[2]+"|"+source[3]+"|"+source[4];
        })()
// case: overlapping copy runs backward
(function(){
            var source=[0,1,2,3,4],result=source.copyWithin(1,0,4);
            return (result===source)+"|"+source[0]+"|"+source[1]+"|"+
                source[2]+"|"+source[3]+"|"+source[4];
        })()
// case: a source hole deletes the corresponding target property
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=[0,1,2,3];delete source[2];source.copyWithin(0,2,4);
            return source.length+"|"+own(source,0)+"|"+(source[0]===undefined)+"|"+
                own(source,1)+"|"+source[1]+"|"+own(source,2)+"|"+source[3];
        })()
// case: an inherited source value is read and written as an own target
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var proto=Object(),source=Object();proto[1]="p";
            Object.setPrototypeOf(source,proto);source[0]="z";source.length=2;
            var result=Array.prototype.copyWithin.call(source,0,1,2);
            return (result===source)+"|"+source[0]+"|"+own(source,0)+"|"+
                source[1]+"|"+own(source,1);
        })()
// BOUND_CASES
// case: target uses saturating Int64 conversion and negative-length offset
(function(){
            function run(target){var a=[0,1,2,3];a.copyWithin(target,2,4);return ""+a[0]+a[1]+a[2]+a[3]}
            return run(undefined)+"|"+run(0/0)+"|"+run(-0)+"|"+run(1.9)+"|"+
                run(-1.9)+"|"+run(-9)+"|"+run(1/0)+"|"+run(-1/0)+"|"+run("1");
        })()
// case: start and end use the pinned clamp and explicit undefined end means length
(function(){
            function startRun(start){var a=[0,1,2,3];a.copyWithin(0,start);return ""+a[0]+a[1]+a[2]+a[3]}
            function endRun(end){var a=[0,1,2,3];a.copyWithin(0,2,end);return ""+a[0]+a[1]+a[2]+a[3]}
            return startRun(undefined)+"|"+startRun(1.9)+"|"+startRun(-1.9)+"|"+
                startRun(1/0)+"|"+startRun(-1/0)+"|"+endRun(undefined)+"|"+
                endRun(0/0)+"|"+endRun(3.9)+"|"+endRun(-1.9)+"|"+endRun(1/0);
        })()
// case: zero length still converts target start and explicit end
(function(){
            var log="",source=Object(),target=Object(),start=Object(),end=Object();
            source.__defineGetter__("length",function(){log+="L";return 0});
            target.valueOf=function(){log+="T";return 0};
            start.valueOf=function(){log+="S";return 0};
            end.valueOf=function(){log+="E";return 0};
            var result=Array.prototype.copyWithin.call(source,target,start,end);
            return (result===source)+"|"+log;
        })()
// case: MAX_SAFE_INTEGER length supports a narrow high-index copy
(function(){
            var source=Object();source.length=9007199254740991;
            source[9007199254740990]="x";
            var result=Array.prototype.copyWithin.call(
                source,9007199254740989,9007199254740990);
            return (result===source)+"|"+source[9007199254740989]+"|"+
                Object.prototype.hasOwnProperty.call(source,"9007199254740989");
        })()
// ORDER_CASES
// case: length target start end and backward indexed operations use pinned order
(function(){
            var log="",source=Object(),length=Object(),target=Object(),start=Object(),end=Object();
            source.__defineGetter__("length",function(){log+="L";return length});
            length.valueOf=function(){log+="N";return 3};
            target.valueOf=function(){log+="T";return 1};
            start.valueOf=function(){log+="S";return 0};
            end.valueOf=function(){log+="E";return 2};
            source.__defineGetter__("0",function(){log+="g0";return "a"});
            source.__defineGetter__("1",function(){log+="g1";return "b"});
            source.__defineSetter__("1",function(value){log+="s1"+value});
            source.__defineSetter__("2",function(value){log+="s2"+value});
            var result=Array.prototype.copyWithin.call(source,target,start,end);
            return (result===source)+"|"+log;
        })()
// case: a backward Set failure preserves the earlier high-index write
(function(){
            var source=[0,1,2],descriptor=Object();
            descriptor.value=1;descriptor.writable=false;descriptor.configurable=true;
            Object.defineProperty(source,"1",descriptor);
            try{source.copyWithin(1,0,2);return "missing"}
            catch(error){return source[0]+"|"+source[1]+"|"+source[2]+"|"+
                error.name+"|"+error.message}
        })()
// case: a Delete failure preserves an earlier forward write
(function(){
            var source=[0,1,9,3],descriptor=Object();delete source[3];
            descriptor.value=1;descriptor.writable=true;descriptor.configurable=false;
            Object.defineProperty(source,"1",descriptor);
            try{source.copyWithin(0,2,4);return "missing"}
            catch(error){return source[0]+"|"+source[1]+"|"+
                error.name+"|"+error.message}
        })()
// case: a later source getter throw preserves the first backward write
(function(){
            var source=Object();source.length=3;source[2]=2;
            source.__defineGetter__("0",function(){throw 73});
            source.__defineGetter__("1",function(){return 7});
            try{Array.prototype.copyWithin.call(source,1,0,2);return "missing"}
            catch(error){return source[2]+"|"+typeof error+"|"+error}
        })()
// case: target and start throws short-circuit later conversions
(function(){
            function run(which){
                var log="",source=Object(),target=Object(),start=Object(),end=Object();source.length=1;
                target.valueOf=function(){log+="T";if(which===0)throw 81;return 0};
                start.valueOf=function(){log+="S";if(which===1)throw 82;return 0};
                end.valueOf=function(){log+="E";return 1};
                try{Array.prototype.copyWithin.call(source,target,start,end);return "missing"}
                catch(error){return typeof error+":"+error+":"+log}
            }
            return run(0)+"|"+run(1);
        })()
// GENERIC_CASES
// case: ordinary array-like receiver is mutated in place
(function(){
            var source=Object();source[0]="a";source[1]="b";source[2]="c";source.length=3;
            var result=Array.prototype.copyWithin.call(source,0,1);
            return (result===source)+"|"+source[0]+"|"+source[1]+"|"+source[2]+"|"+source.length;
        })()
// case: String indexed targets reject the first attempted Set
(function(){
            try{Array.prototype.copyWithin.call("abc",0,1,2);return "missing"}
            catch(error){return error.name+"|"+error.message}
        })()
// case: primitive receivers with zero length return their wrappers
(function(){
            var number=Array.prototype.copyWithin.call(7,0,0);
            var boolean=Array.prototype.copyWithin.call(true,0,0);
            return Object.prototype.toString.call(number)+"|"+
                Object.prototype.toString.call(boolean)+"|"+typeof number+"|"+typeof boolean;
        })()
// ERROR_CASES
// case: null receiver
Array.prototype.copyWithin.call(null,0,0)
// case: undefined receiver
Array.prototype.copyWithin.call(undefined,0,0)
// case: Symbol target
[0].copyWithin(Symbol('target'),0)
// case: Symbol start
[0].copyWithin(0,Symbol('start'))
// case: Symbol end
[0].copyWithin(0,0,Symbol('end'))
// case: BigInt target
[0].copyWithin(0n,0)
// case: BigInt start
[0].copyWithin(0,0n)
// case: BigInt end
[0].copyWithin(0,0,1n)
// case: Symbol length
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.copyWithin.call(source,0,0)})()
