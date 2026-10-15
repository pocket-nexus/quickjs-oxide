// VALUE_CASES
// case: dense reverse mutates in place while toReversed leaves its source unchanged
(function(){
            var source=[1,2,3,4],same=source.reverse()===source;
            var copy=source.toReversed();
            return same+"|"+source.length+"|"+source[0]+source[1]+source[2]+source[3]+"|"+
                copy.length+"|"+copy[0]+copy[1]+copy[2]+copy[3]+"|"+
                source[0]+source[1]+source[2]+source[3]+"|"+Array.isArray(copy);
        })()
// case: toReversed copies inherited values and densifies holes
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            Array.prototype[2]="inherited";
            var source=Array(4);source[0]="a";source[3]="d";
            var result=source.toReversed();
            var observation=result.length+"|"+result[0]+"|"+result[1]+"|"+
                (result[2]===undefined)+"|"+result[3]+"|"+
                own(result,0)+own(result,1)+own(result,2)+own(result,3)+"|"+
                own(source,1)+own(source,2);
            delete Array.prototype[2];
            return observation;
        })()
// case: sparse reverse preserves holes through all presence combinations
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=Array(7);source[0]="a";source[2]="c";source[6]="g";
            source.reverse();
            return source.length+"|"+source[0]+"|"+own(source,1)+"|"+
                source[4]+"|"+own(source,5)+"|"+source[6]+"|"+
                own(source,0)+own(source,2)+own(source,4)+own(source,6);
        })()
// REVERSE_ORDER_CASES
// case: reverse reads both ends before applying the four presence branches
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=Object(),descriptor=Object(),log="",lowerWrite,upperWrite;
            source.length=8;
            descriptor.get=function(){log+="g0";return "L0"};
            descriptor.set=function(value){log+="s0"+value;lowerWrite=value};
            descriptor.enumerable=true;descriptor.configurable=true;
            Object.defineProperty(source,"0",descriptor);
            descriptor=Object();
            descriptor.get=function(){log+="g7";return "U7"};
            descriptor.set=function(value){log+="s7"+value;upperWrite=value};
            descriptor.enumerable=true;descriptor.configurable=true;
            Object.defineProperty(source,"7",descriptor);
            source[6]="U6";
            source[2]="L2";
            var same=Array.prototype.reverse.call(source)===source;
            return same+"|"+log+"|"+lowerWrite+"|"+upperWrite+"|"+
                own(source,1)+":"+source[1]+":"+own(source,6)+"|"+
                own(source,2)+":"+own(source,5)+":"+source[5]+"|"+
                own(source,3)+":"+own(source,4);
        })()
// case: failed second Set preserves the first Set
(function(){
            var source=Object(),descriptor=Object();source.length=2;source[0]="lower";
            descriptor.value="upper";descriptor.writable=false;
            descriptor.enumerable=true;descriptor.configurable=false;
            Object.defineProperty(source,"1",descriptor);
            try{Array.prototype.reverse.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+source[0]+"|"+source[1]}
        })()
// case: failed upper Delete preserves the lower Set
(function(){
            var source=Object(),descriptor=Object();source.length=2;
            descriptor.value="upper";descriptor.writable=true;
            descriptor.enumerable=true;descriptor.configurable=false;
            Object.defineProperty(source,"1",descriptor);
            try{Array.prototype.reverse.call(source);return "missing"}
            catch(error){
                return error.name+"|"+error.message+"|"+source[0]+"|"+source[1]+"|"+
                    Object.prototype.hasOwnProperty.call(source,"0");
            }
        })()
// case: reverse snapshots length before indexed getters mutate it
(function(){
            var source=Object(),descriptor=Object(),reported=3,log="",lowWrite,highWrite;
            descriptor.get=function(){log+="L";return reported};descriptor.configurable=true;
            Object.defineProperty(source,"length",descriptor);
            descriptor=Object();
            descriptor.get=function(){log+="g0";reported=5;source[4]="ignored";return "low"};
            descriptor.set=function(value){log+="s0";lowWrite=value};descriptor.configurable=true;
            Object.defineProperty(source,"0",descriptor);
            descriptor=Object();
            descriptor.get=function(){log+="g2";return "high"};
            descriptor.set=function(value){log+="s2";highWrite=value};descriptor.configurable=true;
            Object.defineProperty(source,"2",descriptor);
            Array.prototype.reverse.call(source);
            var beforeLengthRead=log;
            return beforeLengthRead+"|"+source.length+"|"+log+"|"+
                lowWrite+"|"+highWrite+"|"+source[4]+"|"+(3 in source);
        })()
// case: MAX_SAFE length uses the full u64 first pair before failing
(function(){
            var source=Object(),descriptor=Object(),log="";source.length=9007199254740991;
            descriptor.get=function(){log+="L";return "low"};descriptor.configurable=true;
            Object.defineProperty(source,"0",descriptor);
            descriptor=Object();
            descriptor.get=function(){log+="U";return "upper"};descriptor.configurable=true;
            Object.defineProperty(source,"9007199254740990",descriptor);
            try{Array.prototype.reverse.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// COPY_ORDER_CASES
// case: toReversed performs descending Has and Get queries
(function(){
            var source=Object(),log="";source.length=4;
            source.__defineGetter__("0",function(){log+="0";return "a"});
            source.__defineGetter__("1",function(){log+="1";return "b"});
            source.__defineGetter__("2",function(){log+="2";return "c"});
            source.__defineGetter__("3",function(){log+="3";return "d"});
            var result=Array.prototype.toReversed.call(source);
            return log+"|"+result[0]+result[1]+result[2]+result[3];
        })()
// case: descending getters observe mutations from earlier high indices
(function(){
            var source=Object(),log="";source.length=3;source[0]="old";source[1]="middle";
            source.__defineGetter__("2",function(){log+="2";source[0]="new";return "tail"});
            source.__defineGetter__("1",function(){log+="1";return "middle"});
            var result=Array.prototype.toReversed.call(source);
            return log+"|"+result[0]+"|"+result[1]+"|"+result[2]+"|"+source[0];
        })()
// case: constructor and species are not consulted
(function(){
            var log="",first=[1,2],second=[3,4],ctor=Object();
            ctor.__defineGetter__(Symbol.species,function(){log+="s";throw 81});
            first.constructor=ctor;
            second.__defineGetter__("constructor",function(){log+="c";throw 82});
            var a=first.toReversed(),b=second.toReversed();
            return a[0]+"|"+a[1]+"|"+b[0]+"|"+b[1]+"|"+log+"|"+
                Array.isArray(a)+"|"+Array.isArray(b);
        })()
// case: INT32_MAX plus one fails before indexed access
(function(){
            var source=Object(),log="";source.length=2147483648;
            source.__defineGetter__("2147483647",function(){log+="H";throw 83});
            source.__defineGetter__("0",function(){log+="L";throw 84});
            try{Array.prototype.toReversed.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// GENERIC_CASES
// case: ordinary array-like receivers support both methods
(function(){
            var source=Object();source[0]="a";source[2]="c";source.length=3;
            var copy=Array.prototype.toReversed.call(source);
            var same=Array.prototype.reverse.call(source)===source;
            return copy.length+"|"+copy[0]+"|"+(copy[1]===undefined)+"|"+copy[2]+"|"+
                same+"|"+source[0]+"|"+(source[1]===undefined)+"|"+source[2];
        })()
// case: number and boolean primitives are boxed or copied in the defining realm
(function(){
            var reversed=Array.prototype.reverse.call(7);
            var copied=Array.prototype.toReversed.call(false);
            return Object.prototype.toString.call(reversed)+"|"+reversed.valueOf()+"|"+
                Object.prototype.hasOwnProperty.call(reversed,"length")+"|"+
                Array.isArray(copied)+"|"+copied.length;
        })()
// case: String reverse rejects immutable indices while toReversed copies UTF-16 units
(function(){
            var errorText;
            try{Array.prototype.reverse.call("ab");errorText="missing"}
            catch(error){errorText=error.name+":"+error.message}
            var result=Array.prototype.toReversed.call("A\uD83D\uDCA9Z");
            return errorText+"|"+result.length+"|"+
                result[0].charCodeAt(0)+","+result[1].charCodeAt(0)+","+
                result[2].charCodeAt(0)+","+result[3].charCodeAt(0)+"|"+
                Object.prototype.hasOwnProperty.call(result,"0");
        })()
// ERROR_CASES
// case: reverse null receiver
Array.prototype.reverse.call(null)
// case: toReversed undefined receiver
Array.prototype.toReversed.call(undefined)
// case: reverse Symbol length
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.reverse.call(source)})()
// case: toReversed BigInt length
(function(){var source=Object();source.length=1n;return Array.prototype.toReversed.call(source)})()
// case: reverse getter user throw
(function(){var source=Object();source.length=2;source.__defineGetter__('0',function(){throw 91});return Array.prototype.reverse.call(source)})()
// case: toReversed getter user throw
(function(){var source=Object();source.length=2;source.__defineGetter__('1',function(){throw 92});return Array.prototype.toReversed.call(source)})()
