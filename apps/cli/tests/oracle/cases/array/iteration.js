// VALUE_CASES
// case: every returns true or short-circuits false
(function(){
            var log="",source=[2,4,5,8];
            var first=source.every(function(value,index){log+="a"+index;return value%2===0});
            var second=[2,4,8].every(function(value,index){log+="b"+index;return value%2===0});
            return first+"|"+second+"|"+log;
        })()
// case: some returns true or exhausts to false
(function(){
            var log="",source=[1,3,4,6];
            var first=source.some(function(value,index){log+="a"+index;return value%2===0});
            var second=[1,3].some(function(value,index){log+="b"+index;return value%2===0});
            return first+"|"+second+"|"+log;
        })()
// case: forEach visits all present elements and returns undefined
(function(){
            var log="",source=[3,4,5];
            var result=source.forEach(function(value,index){log+=index+":"+value+",";return true});
            return (result===undefined)+"|"+log;
        })()
// case: empty receivers use vacuous results without callbacks
(function(){
            var calls=0,callback=function(){calls++;return true};
            return [].every(callback)+"|"+[].some(callback)+"|"+
                ([].forEach(callback)===undefined)+"|"+calls;
        })()
// ORDER_CASES
// case: holes are skipped while inherited values are visited
(function(){
            var log="",proto=Object(),source=Object();proto[1]="p";
            Object.setPrototypeOf(source,proto);source[2]="z";source.length=4;
            source.forEach(function(value,index){log+=index+":"+value+","});
            return log;
        })()
// case: length is snapshotted and later HasProperty observes mutation
(function(){
            var source=[0,1,2],log="";
            source.forEach(function(value,index){
                log+=index+":"+value+",";
                if(index===0){delete source[1];source[3]=3;source.length=4}
            });
            return log+"|"+source.length+"|"+source[3];
        })()
// case: callback value index boxed receiver and strict thisArg are exact
(function(){
            var marker=Object(),source="ab",log="";
            var result=Array.prototype.every.call(source,function(value,index,receiver){
                "use strict";
                log+=(this===marker)+":"+value+":"+index+":"+
                    (typeof receiver)+":"+Object.prototype.toString.call(receiver)+":"+
                    (receiver===source)+",";
                return true;
            },marker);
            return result+"|"+log;
        })()
// case: every and some short-circuit before a later getter
(function(){
            var first=[0,1],second=[1,0],log="";
            first.__defineGetter__("1",function(){log+="E";throw 61});
            second.__defineGetter__("1",function(){log+="S";throw 62});
            var every=first.every(function(value,index){log+="e"+index;return false});
            var some=second.some(function(value,index){log+="s"+index;return true});
            return every+"|"+some+"|"+log;
        })()
// case: callback result uses ToBoolean without invoking object hooks
(function(){
            var log="",truthy=Object();
            truthy.valueOf=function(){log+="v";throw 63};
            truthy.toString=function(){log+="s";throw 64};
            var every=[1].every(function(){return truthy});
            var some=[1].some(function(){return truthy});
            var count=0;[1,2].forEach(function(){count++;return truthy});
            return every+"|"+some+"|"+count+"|"+log;
        })()
// case: length precedes callback validation even for an empty receiver
(function(){
            var log="",source=Object();
            source.__defineGetter__("length",function(){log+="L";return 0});
            try{Array.prototype.every.call(source,7);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: a getter throw precedes callback invocation
(function(){
            var log="",source=Object();source.length=1;
            source.__defineGetter__("0",function(){log+="G";throw 71});
            try{Array.prototype.some.call(source,function(){log+="C";return true});return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// GENERIC_CASES
// case: ordinary array-like receivers pass their object identity
(function(){
            var source=Object();source[0]="a";source[2]="c";source.length=3;
            var log="",result=Array.prototype.forEach.call(source,function(value,index,receiver){
                log+=(receiver===source)+":"+index+":"+value+",";
            });
            return (result===undefined)+"|"+log;
        })()
// case: String receiver iterates UTF-16 units through a boxed callback receiver
(function(){
            var source="A\uD83D\uDCA9Z",sum=0,boxed=true;
            var result=Array.prototype.every.call(source,function(value,index,receiver){
                sum+=value.charCodeAt(0);boxed=boxed&&typeof receiver==="object";
                return index<3;
            });
            return result+"|"+sum+"|"+boxed;
        })()
// case: zero-length primitive receivers still validate callbacks
(function(){
            var calls=0,callback=function(){calls++;return true};
            var number=Array.prototype.every.call(7,callback);
            var boolean=Array.prototype.some.call(false,callback);
            return number+"|"+boolean+"|"+calls;
        })()
// case: MAX_SAFE_INTEGER length can short-circuit at index zero
(function(){
            var source=Object(),calls=0;source.length=9007199254740991;source[0]="x";
            var every=Array.prototype.every.call(source,function(value,index,receiver){
                calls++;return !(receiver===source&&index===0&&value==="x");
            });
            var some=Array.prototype.some.call(source,function(value,index,receiver){
                calls++;return receiver===source&&index===0&&value==="x";
            });
            return every+"|"+some+"|"+calls;
        })()
// ERROR_CASES
// case: null receiver
Array.prototype.every.call(null,function(){return true})
// case: undefined receiver
Array.prototype.some.call(undefined,function(){return true})
// case: missing callback
[].forEach()
// case: undefined callback
[].every(undefined)
// case: number callback
[].some(1)
// case: symbol callback
[].forEach(Symbol('callback'))
// case: BigInt callback
[].every(0n)
// case: Symbol length wins before callback validation
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.some.call(source,1)})()
