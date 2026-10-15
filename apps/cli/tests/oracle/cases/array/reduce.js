// VALUE_CASES
// case: reduce and reduceRight fold in opposite directions
(function(){
            var source=[1,2,3],forward="",reverse="";
            var left=source.reduce(function(acc,value,index){
                forward+=index;return acc*10+value;
            },0);
            var right=source.reduceRight(function(acc,value,index){
                reverse+=index;return acc*10+value;
            },0);
            return left+"|"+right+"|"+forward+"|"+reverse;
        })()
// case: omitted initial value is selected without a callback
(function(){
            var source=[1,2,3],left="",right="";
            var forward=source.reduce(function(acc,value,index){
                left+=index;return acc+value;
            });
            var reverse=source.reduceRight(function(acc,value,index){
                right+=index;return acc+value;
            });
            return forward+"|"+reverse+"|"+left+"|"+right;
        })()
// case: explicit undefined is an initial value even for an empty receiver
(function(){
            var calls=0,callback=function(acc,value){
                calls++;return (acc===undefined?"undefined":acc)+value;
            };
            var empty=[].reduce(callback,undefined);
            var one=[5].reduce(callback,undefined);
            return (empty===undefined)+"|"+one+"|"+calls;
        })()
// case: a single present element needs no callback when initial value is omitted
(function(){
            var source=Array(4),calls=0;source[2]=7;
            var left=source.reduce(function(){calls++;return 0});
            var right=source.reduceRight(function(){calls++;return 0});
            return left+"|"+right+"|"+calls;
        })()
// case: a present undefined element is still a valid omitted accumulator
(function(){
            var source=Array(3),calls=0;source[1]=undefined;
            var left=source.reduce(function(){calls++;return 1});
            var right=source.reduceRight(function(){calls++;return 2});
            return (left===undefined)+"|"+(right===undefined)+"|"+calls;
        })()
// case: each callback result becomes the next accumulator without coercion
(function(){
            var result=[1,2,3].reduce(function(acc,value,index){
                return "("+acc+":"+value+":"+index+")";
            },"x");
            return result;
        })()
// ORDER_CASES
// case: holes are skipped while inherited values participate
(function(){
            var proto=Object(),source=Object(),log="";proto[1]=4;
            Object.setPrototypeOf(source,proto);source[3]=8;source.length=5;
            var left=Array.prototype.reduce.call(source,function(acc,value,index){
                log+="l"+index+":"+value+",";return acc+value;
            },1);
            var right=Array.prototype.reduceRight.call(source,function(acc,value,index){
                log+="r"+index+":"+value+",";return acc+value;
            },1);
            return left+"|"+right+"|"+log;
        })()
// case: length is snapshotted while later HasProperty observes mutation
(function(){
            var source=[1,2,3],log="";
            var result=source.reduce(function(acc,value,index){
                log+=index+":"+value+",";
                if(index===0){delete source[1];source[3]=4;source.length=4}
                return acc+value;
            },0);
            return result+"|"+log+"|"+source.length+"|"+source[3];
        })()
// case: callback accumulator value index receiver and strict this are exact
(function(){
            var source=[5],initial=Object(),log="";
            var result=source.reduce(function(acc,value,index,receiver){
                "use strict";
                log+=(this===undefined)+":"+(acc===initial)+":"+value+":"+index+":"+
                    (receiver===source);
                return receiver;
            },initial);
            return (result===source)+"|"+log;
        })()
// case: omitted initial scan and callback follow directional Get order
(function(){
            var source=Object(),log="";source.length=4;
            source.__defineGetter__("1",function(){log+="g1,";return 2});
            source.__defineGetter__("3",function(){log+="g3,";return 4});
            var left=Array.prototype.reduce.call(source,function(acc,value,index){
                log+="c"+index+",";return acc+value;
            });
            log+="|";
            var right=Array.prototype.reduceRight.call(source,function(acc,value,index){
                log+="c"+index+",";return acc+value;
            });
            return left+"|"+right+"|"+log;
        })()
// case: length access precedes callback validation
(function(){
            var source=Object(),log="";
            source.__defineGetter__("length",function(){log+="L";return 0});
            try{Array.prototype.reduce.call(source,7);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: a value getter throw precedes callback invocation
(function(){
            var source=Object(),log="";source.length=1;
            source.__defineGetter__("0",function(){log+="G";throw 71});
            try{Array.prototype.reduce.call(source,function(){log+="C";return 0},0);return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// case: a callback throw stops before later indexed access
(function(){
            var source=[1,2],log="";
            source.__defineGetter__("1",function(){log+="G";throw 72});
            try{source.reduce(function(){log+="C";throw 73},0);return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// GENERIC_CASES
// case: ordinary array-like receivers pass their object identity
(function(){
            var source=Object();source[0]="a";source[2]="c";source.length=3;
            return Array.prototype.reduce.call(source,function(acc,value,index,receiver){
                return acc+(receiver===source)+":"+index+":"+value+",";
            },"");
        })()
// case: String receivers expose UTF-16 units through a boxed receiver
(function(){
            var source="A\uD83D\uDCA9Z",boxed=true,indices="";
            var sum=Array.prototype.reduce.call(source,function(acc,value,index,receiver){
                boxed=boxed&&typeof receiver==="object"&&receiver!==source;
                indices+=index;return acc+value.charCodeAt(0);
            },0);
            return sum+"|"+indices+"|"+boxed;
        })()
// case: zero-length primitive receivers return an explicit initial value
(function(){
            var calls=0,callback=function(){calls++;return 0};
            var number=Array.prototype.reduce.call(7,callback,"n");
            var boolean=Array.prototype.reduceRight.call(false,callback,"b");
            return number+"|"+boolean+"|"+calls;
        })()
// case: MAX_SAFE_INTEGER endpoints can abort after one callback
(function(){
            var source=Object(),log="";source.length=9007199254740991;
            source[0]="l";source[9007199254740990]="r";
            try{Array.prototype.reduce.call(source,function(acc,value,index){
                log+="L"+index+value;throw 81;
            },0)}catch(error){log+="="+error}
            try{Array.prototype.reduceRight.call(source,function(acc,value,index){
                log+="R"+index+value;throw 82;
            },0)}catch(error){log+="="+error}
            return log;
        })()
// ERROR_CASES
// case: null receiver
Array.prototype.reduce.call(null,function(){return 0},0)
// case: undefined receiver
Array.prototype.reduceRight.call(undefined,function(){return 0},0)
// case: missing callback
[1].reduce()
// case: undefined callback
[1].reduceRight(undefined)
// case: number callback
[].reduce(1)
// case: symbol callback
[1].reduce(Symbol('callback'))
// case: BigInt callback
[1].reduceRight(0n)
// case: Symbol length wins before callback validation
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.reduce.call(source,1)})()
// case: empty reduce without initial value
[].reduce(function(){})
// case: empty reduceRight without initial value
[].reduceRight(function(){})
// case: all-hole receiver without initial value
Array(3).reduce(function(){})
