// VALUE_CASES
// case: dense mutators return the removed value or new length
(function(){
            var source=[1,2,3];
            var popped=source.pop(),shifted=source.shift();
            var pushed=source.push(4,5),unshifted=source.unshift(0,-1);
            return popped+"|"+shifted+"|"+pushed+"|"+unshifted+"|"+
                source.length+"|"+source[0]+"|"+source[1]+"|"+source[2]+"|"+
                source[3]+"|"+source[4];
        })()
// case: shift and pop preserve sparse holes while removing endpoints
(function(){
            var source=Array(4);source[1]="b";source[3]="d";
            var shifted=source.shift(),popped=source.pop();
            return (shifted===undefined)+"|"+popped+"|"+source.length+"|"+
                source[0]+"|"+(1 in source)+"|"+(2 in source)+"|"+
                Object.prototype.hasOwnProperty.call(source,"0");
        })()
// case: push distinguishes no arguments from explicit undefined
(function(){
            var empty=[],zero=empty.push(),one=empty.push(undefined);
            var ignored=[7,8],removed=Array.prototype.pop.call(ignored,1,2,3);
            return zero+"|"+one+"|"+empty.length+"|"+(0 in empty)+"|"+
                (empty[0]===undefined)+"|"+removed+"|"+ignored.length+"|"+ignored[0];
        })()
// case: unshift inserts actual arguments before sparse existing elements
(function(){
            var source=Array(3);source[1]="middle";
            var length=source.unshift("first",undefined);
            return length+"|"+source.length+"|"+source[0]+"|"+
                (source[1]===undefined)+"|"+
                Object.prototype.hasOwnProperty.call(source,"1")+"|"+
                (2 in source)+"|"+source[3]+"|"+(4 in source);
        })()
// GENERIC_CASES
// case: ordinary array-like objects support all four generic mutators
(function(){
            var source=Object();source[0]="a";source[1]="b";source.length=2;
            var popped=Array.prototype.pop.call(source);
            var pushed=Array.prototype.push.call(source,"c");
            var shifted=Array.prototype.shift.call(source);
            var unshifted=Array.prototype.unshift.call(source,"x","y");
            return popped+"|"+pushed+"|"+shifted+"|"+unshifted+"|"+
                source.length+"|"+source[0]+"|"+source[1]+"|"+source[2];
        })()
// case: number primitives are boxed for indexed and length Sets
(function(){
            var captured,value,log="";Number.prototype.length=0;
            Number.prototype.__defineSetter__("0",function(argument){
                captured=this;value=argument;log+="I";
            });
            var result=Array.prototype.push.call(7,"x");
            return result+"|"+Object.prototype.toString.call(captured)+"|"+
                captured.valueOf()+"|"+(value==="x")+"|"+captured.length+"|"+
                Object.prototype.hasOwnProperty.call(captured,"length")+"|"+
                Object.prototype.hasOwnProperty.call(captured,"0")+"|"+log;
        })()
// case: String push writes through a prototype setter before final length fails
(function(){
            var captured,log="",stringPrototype=Object.getPrototypeOf(Object("ab"));
            stringPrototype.__defineSetter__("2",function(value){
                captured=this;log+="S"+value;
            });
            try{Array.prototype.push.call("ab","c");return "missing"}
            catch(error){
                return error.name+"|"+error.message+"|"+log+"|"+
                    Object.prototype.toString.call(captured)+"|"+captured[0]+captured[1]+"|"+
                    captured.length+"|"+Object.prototype.hasOwnProperty.call(captured,"2");
            }
        })()
// case: empty primitive receivers still receive a final zero length Set
(function(){
            var numberBox,booleanBox,log="";
            Number.prototype.length=0;Boolean.prototype.length=0;
            Number.prototype.__defineSetter__("length",function(value){numberBox=this;log+="N"+value});
            Boolean.prototype.__defineSetter__("length",function(value){booleanBox=this;log+="B"+value});
            var first=Array.prototype.pop.call(3),second=Array.prototype.shift.call(false);
            return (first===undefined)+"|"+(second===undefined)+"|"+
                Object.prototype.toString.call(numberBox)+"|"+
                Object.prototype.toString.call(booleanBox)+"|"+log;
        })()
// LENGTH_AND_ORDER_CASES
// case: push reads and coerces length before ascending Sets and final length
(function(){
            var source=Object(),lengthValue=Object(),first=Object(),log="",descriptor=Object();
            lengthValue.valueOf=function(){log+="N";return 1.9};
            first.valueOf=function(){log+="V";throw 61};
            first.toString=function(){log+="T";throw 62};
            descriptor.get=function(){log+="L";return lengthValue};
            descriptor.set=function(value){log+="F"+value};
            Object.defineProperty(source,"length",descriptor);
            source.__defineSetter__("1",function(value){log+="1"+(value===first)});
            source.__defineSetter__("2",function(value){log+="2"+(value===undefined)});
            var result=Array.prototype.push.call(source,first,undefined);
            return result+"|"+log;
        })()
// case: push with no arguments still performs length Get conversion and Set
(function(){
            var source=Object(),number=Object(),descriptor=Object(),log="";
            number.valueOf=function(){log+="N";return 2.8};
            descriptor.get=function(){log+="L";return number};
            descriptor.set=function(value){log+="S"+value};
            Object.defineProperty(source,"length",descriptor);
            return Array.prototype.push.call(source)+"|"+log;
        })()
// case: push through a Proxy still exposes the final length Set
(function(){
            var target=[],log=[],handler=Object();
            handler.get=function(target,key,receiver){
                log.push("get:"+key);return Reflect.get(target,key,receiver);
            };
            handler.set=function(target,key,value,receiver){
                log.push("set:"+key+":"+value);
                return Reflect.set(target,key,value,receiver);
            };
            var proxy=new Proxy(target,handler);
            var result=Array.prototype.push.call(proxy,"x");
            return result+"|"+target.length+"|"+target[0]+"|"+log.join(",");
        })()
// case: empty pop converts length then Sets zero without indexed access
(function(){
            var source=Object(),number=Object(),descriptor=Object(),log="";
            number.valueOf=function(){log+="N";return -4};
            descriptor.get=function(){log+="L";return number};
            descriptor.set=function(value){log+="S"+value};
            Object.defineProperty(source,"length",descriptor);
            source.__defineGetter__("0",function(){log+="G";throw 71});
            var result=Array.prototype.pop.call(source);
            return (result===undefined)+"|"+log;
        })()
// case: fractional NaN and infinite lengths use QuickJS ToLength
(function(){
            function pushed(length){var source=Object();source.length=length;return Array.prototype.push.call(source)}
            function popped(length){var source=Object();source.length=length;return Array.prototype.pop.call(source)}
            return pushed(2.9)+"|"+pushed(0/0)+"|"+pushed(-1)+"|"+
                popped(0/0)+"|"+popped(-1)+"|"+popped(1/0);
        })()
// MOVE_CASES
// case: shift uses Has Get Set and Delete in ascending order
(function(){
            var source=Object(),proto=Object(),descriptor=Object(),log="";
            Object.setPrototypeOf(source,proto);
            descriptor.get=function(){log+="L";return 4};
            descriptor.set=function(value){log+="N"+value};
            Object.defineProperty(source,"length",descriptor);
            descriptor=Object();descriptor.get=function(){log+="R";return "head"};
            descriptor.set=function(value){log+="S0"+value};
            Object.defineProperty(source,"0",descriptor);
            source[1]="delete-me";
            proto.__defineGetter__("3",function(){log+="G3";return "inherited"});
            var result=Array.prototype.shift.call(source);
            return result+"|"+log+"|"+
                Object.prototype.hasOwnProperty.call(source,"1")+"|"+
                Object.prototype.hasOwnProperty.call(source,"2")+"|"+source[2]+"|"+
                (3 in source)+"|"+Object.prototype.hasOwnProperty.call(source,"3");
        })()
// case: unshift copies backwards then writes arguments forwards
(function(){
            var source=Object(),proto=Object(),descriptor=Object(),log="";
            Object.setPrototypeOf(source,proto);source[0]="a";source[3]="delete-me";
            descriptor.get=function(){log+="L";return 3};
            descriptor.set=function(value){log+="N"+value};
            Object.defineProperty(source,"length",descriptor);
            descriptor=Object();descriptor.get=function(){log+="G2";return "p2"};
            descriptor.set=function(value){log+="S2"+value};
            Object.defineProperty(proto,"2",descriptor);
            proto.__defineSetter__("4",function(value){log+="S4"+value});
            var result=Array.prototype.unshift.call(source,"x","y");
            return result+"|"+log+"|"+source[0]+"|"+source[1]+"|"+
                source[2]+"|"+Object.prototype.hasOwnProperty.call(source,"2")+"|"+
                Object.prototype.hasOwnProperty.call(source,"3")+"|"+
                Object.prototype.hasOwnProperty.call(source,"4");
        })()
// case: shift snapshots length while getters can mutate later sources
(function(){
            var source=Object(),moved,descriptor=Object(),log="";source.length=3;
            descriptor.get=function(){log+="G0";source[1]="late";source[3]="ignored";source.length=4;return "head"};
            descriptor.set=function(value){log+="S0";moved=value};
            Object.defineProperty(source,"0",descriptor);
            source[2]="tail";
            var result=Array.prototype.shift.call(source);
            return result+"|"+source.length+"|"+moved+"|"+source[1]+"|"+
                (2 in source)+"|"+source[3]+"|"+log;
        })()
// PARTIAL_AND_LIMIT_CASES
// case: failed unshift Set preserves the already moved suffix
(function(){
            var source=Object(),descriptor=Object();
            source[0]="a";source[1]="b";source[2]="c";source.length=3;
            descriptor.value="locked";descriptor.writable=false;
            descriptor.enumerable=true;descriptor.configurable=false;
            Object.defineProperty(source,"3",descriptor);
            try{Array.prototype.unshift.call(source,"x","y");return "missing"}
            catch(error){
                return error.name+"|"+error.message+"|"+source.length+"|"+
                    source[0]+"|"+source[1]+"|"+source[2]+"|"+source[3]+"|"+source[4];
            }
        })()
// case: failed shift Delete preserves earlier Set and skips final length
(function(){
            var source=Object(),descriptor=Object();source[0]="a";source.length=3;
            descriptor.value="b";descriptor.writable=true;
            descriptor.enumerable=true;descriptor.configurable=false;
            Object.defineProperty(source,"1",descriptor);
            try{Array.prototype.shift.call(source);return "missing"}
            catch(error){
                return error.name+"|"+error.message+"|"+source.length+"|"+
                    source[0]+"|"+source[1]+"|"+(2 in source);
            }
        })()
// case: failed final length Set leaves a pushed indexed property
(function(){
            var source=Object(),descriptor=Object();source[0]="a";
            descriptor.value=1;descriptor.writable=false;
            descriptor.enumerable=true;descriptor.configurable=true;
            Object.defineProperty(source,"length",descriptor);
            try{Array.prototype.push.call(source,"b");return "missing"}
            catch(error){
                return error.name+"|"+error.message+"|"+source.length+"|"+
                    source[0]+"|"+source[1]+"|"+
                    Object.prototype.hasOwnProperty.call(source,"1");
            }
        })()
// case: genuine Array push retains the final non-writable length failure
(function(){
            var source=[],proto=Array.prototype,descriptor=Object(),log="";
            descriptor.configurable=true;
            descriptor.set=function(value){
                log+="S"+value;this.length=1;
                Object.defineProperty(this,"length",{writable:false});
            };
            Object.defineProperty(proto,"0",descriptor);
            try{
                try{source.push("x");return "missing"}
                catch(error){
                    return error.name+"|"+error.message+"|"+source.length+"|"+
                        Object.prototype.hasOwnProperty.call(source,"0")+"|"+log;
                }
            }finally{delete proto[0]}
        })()
// case: genuine Array push restores a length changed by an inherited setter
(function(){
            var source=[],proto=Array.prototype,descriptor=Object(),log="";
            descriptor.configurable=true;
            descriptor.set=function(value){
                log+="S"+value;this.length=3;this[2]="tail";
            };
            Object.defineProperty(proto,"0",descriptor);
            try{
                var result=source.push("x");
                return result+"|"+source.length+"|"+
                    Object.prototype.hasOwnProperty.call(source,"0")+"|"+
                    Object.prototype.hasOwnProperty.call(source,"2")+"|"+log;
            }finally{delete proto[0]}
        })()
// case: MAX_SAFE overflow throws before indexed or final length writes
(function(){
            var source=Object(),descriptor=Object(),log="";
            descriptor.get=function(){log+="L";return 9007199254740991};
            descriptor.set=function(value){log+="N"+value};
            Object.defineProperty(source,"length",descriptor);
            source.__defineSetter__("9007199254740991",function(){log+="W"});
            try{Array.prototype.push.call(source,"x");return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: MAX_SAFE pop and push retain full Int64 property keys
(function(){
            var tail=Object(),head=Object();
            tail.length=9007199254740991;tail[9007199254740990]="last";
            tail[4294967294]="sentinel";
            var popped=Array.prototype.pop.call(tail);
            head.length=9007199254740990;
            var pushed=Array.prototype.push.call(head,"next");
            return popped+"|"+tail.length+"|"+
                Object.prototype.hasOwnProperty.call(tail,"9007199254740990")+"|"+
                tail[4294967294]+"|"+pushed+"|"+head.length+"|"+
                head[9007199254740990]+"|"+
                Object.prototype.hasOwnProperty.call(head,"9007199254740990");
        })()
// case: maximum Uint32 Array length writes an ordinary key before RangeError
(function(){
            var source=[];source.length=4294967295;
            try{source.push("x");return "missing"}
            catch(error){
                return error.name+"|"+error.message+"|"+source.length+"|"+
                    source[4294967295]+"|"+
                    Object.prototype.hasOwnProperty.call(source,"4294967295");
            }
        })()
// case: maximum writable Array index grows length through the Uint32 boundary
(function(){
            var source=[];source.length=4294967294;
            var result=source.push("last");
            return result+"|"+source.length+"|"+source[4294967294]+"|"+
                Object.prototype.hasOwnProperty.call(source,"4294967294");
        })()
// ERROR_CASES
// case: pop null receiver
Array.prototype.pop.call(null)
// case: push undefined receiver
Array.prototype.push.call(undefined,1)
// case: shift null receiver
Array.prototype.shift.call(null)
// case: unshift undefined receiver
Array.prototype.unshift.call(undefined,1)
// case: Symbol length
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.pop.call(source)})()
// case: BigInt length
(function(){var source=Object();source.length=1n;return Array.prototype.push.call(source,2)})()
// case: String pop cannot delete a non-configurable indexed property
Array.prototype.pop.call('ab')
// case: length getter user throw
(function(){var source=Object();source.__defineGetter__('length',function(){throw 91});return Array.prototype.unshift.call(source,1)})()
