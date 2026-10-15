// VALUE_CASES
// case: find and findIndex stop at the first forward match
(function(){
            var source=[1,4,6,8],log="";
            var value=source.find(function(element,index){log+="v"+index;return element>5});
            var index=source.findIndex(function(element,position){log+="i"+position;return element>5});
            return value+"|"+index+"|"+log+"|"+source.length+"|"+source[2];
        })()
// case: findLast and findLastIndex stop at the first reverse match
(function(){
            var source=[1,4,6,8],log="";
            var value=source.findLast(function(element,index){log+="v"+index;return element<7});
            var index=source.findLastIndex(function(element,position){log+="i"+position;return element<7});
            return value+"|"+index+"|"+log;
        })()
// case: not-found results use undefined and minus one
(function(){
            var source=[1,2],never=function(){return false};
            return (source.find(never)===undefined)+"|"+source.findIndex(never)+"|"+
                (source.findLast(never)===undefined)+"|"+source.findLastIndex(never);
        })()
// case: predicate results use ToBoolean without object coercion hooks
(function(){
            var log="",truthy=Object();
            truthy.valueOf=function(){log+="v";throw 61};
            truthy.toString=function(){log+="s";throw 62};
            var value=[3].find(function(){return truthy});
            var index=[3].findIndex(function(){return 0});
            return value+"|"+index+"|"+log;
        })()
// ORDER_CASES
// case: holes are read as undefined and still invoke the predicate
(function(){
            var source=Array(3),log="";source[2]=7;
            var value=source.find(function(element,index){
                log+=index+":"+(element===undefined)+",";return index===1;
            });
            return (value===undefined)+"|"+log;
        })()
// case: inherited values and reverse getter order remain observable
(function(){
            var log="",proto=Object(),source=Object();proto[1]="p";
            Object.setPrototypeOf(source,proto);source.length=3;
            source.__defineGetter__("2",function(){log+="g2";return "z"});
            var value=Array.prototype.findLast.call(source,function(element,index){
                log+="c"+index+element;return element==="p";
            });
            return value+"|"+log;
        })()
// case: length is snapshotted while later Gets observe mutation
(function(){
            var source=[0,1,2],log="";
            var result=source.findIndex(function(element,index){
                log+=index+":"+(element===undefined)+",";
                if(index===0){delete source[1];source[3]=3;source.length=4}
                return false;
            });
            return result+"|"+log+"|"+source.length+"|"+source[3];
        })()
// case: callback arguments thisArg and early stop use pinned identities
(function(){
            var marker=Object(),source=[5,6],log="";
            source.__defineGetter__("1",function(){log+="late";throw 63});
            var value=source.find(function(element,index,receiver){
                "use strict";log+=(this===marker)+":"+(receiver===source)+":"+index;
                return true;
            },marker);
            return value+"|"+log;
        })()
// case: length precedes predicate validation even for an empty receiver
(function(){
            var log="",source=Object();
            source.__defineGetter__("length",function(){log+="L";return 0});
            try{Array.prototype.find.call(source,7);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: a Get throw precedes callback invocation
(function(){
            var log="",source=Object();source.length=1;
            source.__defineGetter__("0",function(){log+="G";throw 71});
            try{Array.prototype.find.call(source,function(){log+="C";return true});return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// GENERIC_CASES
// case: ordinary array-like receivers pass the original object
(function(){
            var source=Object();source[0]="a";source[1]="b";source.length=2;
            var result=Array.prototype.findIndex.call(source,function(value,index,receiver){
                return receiver===source&&value==="b"&&index===1;
            });
            return result;
        })()
// case: String receiver callback sees UTF-16 units and the original primitive
(function(){
            var source="A\uD83D\uDCA9Z",seen="";
            var value=Array.prototype.find.call(source,function(element,index,receiver){
                seen+=typeof receiver+":"+(receiver===source)+":"+index+",";
                return element.charCodeAt(0)===0xDCA9;
            });
            return value.charCodeAt(0)+"|"+seen;
        })()
// case: zero-length primitive receivers validate but do not call predicates
(function(){
            var calls=0,predicate=function(){calls++;return true};
            var number=Array.prototype.find.call(7,predicate);
            var boolean=Array.prototype.findLastIndex.call(true,predicate);
            return (number===undefined)+"|"+boolean+"|"+calls;
        })()
// case: findLast supports one MAX_SAFE_INTEGER high-index Get
(function(){
            var source=Object(),calls=0;source.length=9007199254740991;
            source[9007199254740990]="x";
            var value=Array.prototype.findLast.call(source,function(element,index,receiver){
                calls++;return receiver===source&&index===9007199254740990&&element==="x";
            });
            return value+"|"+calls;
        })()
// ERROR_CASES
// case: null receiver
Array.prototype.find.call(null,function(){return true})
// case: undefined receiver
Array.prototype.find.call(undefined,function(){return true})
// case: missing predicate
[].find()
// case: undefined predicate
[].findIndex(undefined)
// case: number predicate
[].findLast(1)
// case: symbol predicate
[].findLastIndex(Symbol('predicate'))
// case: BigInt predicate
[].find(0n)
// case: Symbol length wins before predicate validation
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.find.call(source,1)})()
