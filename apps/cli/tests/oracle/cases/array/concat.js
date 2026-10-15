// VALUE_CASES
// case: dense Arrays and primitive arguments concatenate left to right
(function(){
            var source=[1,2],other=[3,4];
            var result=source.concat(other,5,"x",null,undefined);
            return result.length+"|"+result[0]+"|"+result[1]+"|"+result[2]+"|"+
                result[3]+"|"+result[4]+"|"+result[5]+"|"+result[6]+"|"+
                (result[7]===undefined)+"|"+(result!==source)+"|"+source.length+"|"+other.length;
        })()
// case: sparse Arrays preserve holes from every spread source
(function(){
            var source=[1,,3],other=[4,,6],result=source.concat(other);
            return result.length+"|"+result[0]+"|"+(1 in result)+"|"+result[2]+"|"+
                result[3]+"|"+(4 in result)+"|"+result[5];
        })()
// case: concat with no arguments copies only the receiver
(function(){
            var source=[1,2],result=source.concat();
            result[0]=9;
            return result.length+"|"+result[0]+"|"+source[0]+"|"+(result!==source);
        })()
// case: explicit undefined remains distinct from no argument
(function(){
            var empty=[].concat(),one=[].concat(undefined);
            return empty.length+"|"+one.length+"|"+(0 in one)+"|"+(one[0]===undefined);
        })()
// case: nested Arrays are flattened by exactly one level
(function(){
            var nested=[[2,3]],result=[1].concat(nested);
            return result.length+"|"+result[0]+"|"+Array.isArray(result[1])+"|"+
                result[1].length+"|"+result[1][0]+"|"+result[1][1];
        })()
// SPREAD_CASES
// case: Arrays can opt out and ordinary objects can opt into spreading
(function(){
            var hidden=[2,3],ordinary=Object(),nulled=[6,7];
            hidden[Symbol.isConcatSpreadable]=false;
            ordinary[0]=4;ordinary[1]=5;ordinary.length=2;
            ordinary[Symbol.isConcatSpreadable]=true;
            nulled[Symbol.isConcatSpreadable]=null;
            var result=[1].concat(hidden,ordinary,nulled);
            return result.length+"|"+result[0]+"|"+(result[1]===hidden)+"|"+
                result[2]+"|"+result[3]+"|"+(result[4]===nulled);
        })()
// case: object spreadability is truthy without coercion hooks
(function(){
            var log="",marker=Object(),source=Object();
            marker.valueOf=function(){log+="v";throw 61};
            marker.toString=function(){log+="s";throw 62};
            source[0]="x";source.length=1;source[Symbol.isConcatSpreadable]=marker;
            var result=[].concat(source);
            return result.length+"|"+result[0]+"|"+log;
        })()
// case: holes skip Get while inherited values become own result properties
(function(){
            var proto=Object.create(Array.prototype),source=Array(3);
            proto[1]="p";source[2]="z";Object.setPrototypeOf(source,proto);
            source.__defineGetter__("0",function(){throw 63});delete source[0];
            var result=source.concat();
            return result.length+"|"+(0 in result)+"|"+result[1]+"|"+result[2]+"|"+
                Object.prototype.hasOwnProperty.call(result,"1");
        })()
// case: species allocation precedes spreadability length and indexed access
(function(){
            var log="",source=[1],ctor=Object(),speciesDescriptor=Object();
            var resultProto=Object();
            resultProto.__defineSetter__("length",function(value){log+="L"+value});
            function Species(length){log+="N"+length;return Object.create(resultProto)}
            speciesDescriptor.get=function(){log+="S";return Species};
            Object.defineProperty(ctor,Symbol.species,speciesDescriptor);
            source.__defineGetter__("constructor",function(){log+="C";return ctor});
            var spreadDescriptor=Object();
            spreadDescriptor.get=function(){log+="I";return undefined};
            Object.defineProperty(source,Symbol.isConcatSpreadable,spreadDescriptor);
            source.__defineGetter__("0",function(){log+="G";return 4});
            var result=source.concat(5);
            return result[0]+"|"+result[1]+"|"+log;
        })()
// case: spread length is snapshotted while later Has and Get see mutation
(function(){
            var source=Object(),log="";source.length=4;
            source[Symbol.isConcatSpreadable]=true;
            source.__defineGetter__("0",function(){
                log+="g0,";delete source[1];source[2]=3;source[4]=5;source.length=5;return 1;
            });
            source[1]=2;source[3]=4;
            var result=[].concat(source);
            return result.length+"|"+result[0]+"|"+(1 in result)+"|"+result[2]+"|"+
                result[3]+"|"+(4 in result)+"|"+log+"|"+source.length;
        })()
// case: an indexed getter throw stops before later arguments and final length Set
(function(){
            var log="",source=Object(),later=Object();source.length=2;
            source[Symbol.isConcatSpreadable]=true;
            source.__defineGetter__("0",function(){log+="G";throw 71});
            later.__defineGetter__(Symbol.isConcatSpreadable,function(){log+="L";return false});
            try{[].concat(source,later);return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// SPECIES_CASES
// case: custom species receive zero and ordinary results get a final length
(function(){
            function Species(length){var result=Object();result.arg=length;return result}
            var ctor=Object(),source=[1,,3];ctor[Symbol.species]=Species;source.constructor=ctor;
            var result=source.concat(4),first=result[0];
            result[0]=8;var writable=result[0]===8;
            var enumerable=Object.prototype.propertyIsEnumerable.call(result,"0");
            var configurable=delete result[0];
            return result.arg+"|"+result.length+"|"+first+"|"+(1 in result)+"|"+
                result[2]+"|"+result[3]+"|"+writable+":"+enumerable+":"+configurable;
        })()
// case: a custom result property under a source hole is not deleted
(function(){
            function Species(){var result=Object();result[1]="seed";return result}
            var ctor=Object(),source=[1,,3];ctor[Symbol.species]=Species;source.constructor=ctor;
            var result=source.concat();
            return result.length+"|"+result[0]+"|"+result[1]+"|"+result[2];
        })()
// case: indexed defines bypass setters while final length uses an inherited setter
(function(){
            var indexHits=0,lengthLog="",proto=Object(),ctor=Object();
            proto.__defineSetter__("0",function(){indexHits++});
            proto.__defineSetter__("length",function(value){lengthLog+="L"+value});
            function Species(){return Object.create(proto)}
            ctor[Symbol.species]=Species;
            var source=[2];source.constructor=ctor;
            var result=source.concat(3);
            return result[0]+"|"+result[1]+"|"+indexHits+"|"+lengthLog+"|"+
                Object.prototype.hasOwnProperty.call(result,"0")+"|"+
                Object.prototype.hasOwnProperty.call(result,"length");
        })()
// case: a rejected indexed definition preserves only the written prefix
(function(){
            var captured,log="",ctor=Object();
            function Species(){
                var result=Object(),descriptor=Object();captured=result;
                descriptor.value=9;descriptor.writable=false;
                descriptor.enumerable=true;descriptor.configurable=false;
                Object.defineProperty(result,"1",descriptor);return result;
            }
            ctor[Symbol.species]=Species;
            var source=[1,2,3];source.constructor=ctor;
            source.__defineGetter__("2",function(){log+="G";throw 72});
            try{source.concat();return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+captured[0]+"|"+
                captured[1]+"|"+log+"|"+("length" in captured)}
        })()
// case: a rejected final length Set leaves all indexed writes observable
(function(){
            var captured,ctor=Object();
            function Species(){
                var result=Object(),descriptor=Object();captured=result;
                descriptor.value=0;descriptor.writable=false;
                descriptor.enumerable=false;descriptor.configurable=false;
                Object.defineProperty(result,"length",descriptor);return result;
            }
            ctor[Symbol.species]=Species;
            var source=[1,2];source.constructor=ctor;
            try{source.concat(3);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+captured[0]+"|"+
                captured[1]+"|"+captured[2]+"|"+captured.length}
        })()
// case: generic receivers ignore constructor while default creation ignores global Array replacement
(function(){
            var intrinsic=Array,concat=Array.prototype.concat,source=Object(),log="";
            source.__defineGetter__("constructor",function(){log+="C";throw 81});
            globalThis.Array=function(){throw 82};
            var result=concat.call(source);
            return intrinsic.isArray(result)+"|"+result.length+"|"+
                (result[0]===source)+"|"+log;
        })()
// GENERIC_CASES
// case: ordinary array-like receivers are single elements unless opted in
(function(){
            var single=Object();single[0]="a";single.length=1;
            var one=Array.prototype.concat.call(single);
            single[Symbol.isConcatSpreadable]=true;
            var spread=Array.prototype.concat.call(single);
            return one.length+"|"+(one[0]===single)+"|"+spread.length+"|"+spread[0];
        })()
// case: primitive String receiver is boxed once and primitive String arguments do not spread
(function(){
            var result=Array.prototype.concat.call("ab","cd");
            return result.length+"|"+(typeof result[0])+"|"+
                Object.prototype.toString.call(result[0])+"|"+result[0]+"|"+
                (typeof result[1])+"|"+result[1];
        })()
// case: boxed String objects can opt into UTF-16 spreading
(function(){
            var source=Object("A\uD83D\uDCA9Z");
            source[Symbol.isConcatSpreadable]=true;
            var result=[].concat(source);
            return result.length+"|"+result[0].charCodeAt(0)+"|"+
                result[1].charCodeAt(0)+"|"+result[2].charCodeAt(0)+"|"+
                result[3].charCodeAt(0);
        })()
// case: negative NaN and fractional spread lengths use ToLength
(function(){
            function make(length,value){
                var source=Object();source.length=length;source[0]=value;
                source[Symbol.isConcatSpreadable]=true;return source;
            }
            var result=[].concat(make(-1,"n"),make(0/0,"z"),make(1.9,"f"));
            return result.length+"|"+result[0];
        })()
// case: MAX_SAFE_INTEGER overflow is checked before indexed access
(function(){
            var receiver=Object(),huge=Object(),log="";
            huge.length=9007199254740991;huge[Symbol.isConcatSpreadable]=true;
            huge.__defineGetter__("0",function(){log+="G";throw 91});
            try{Array.prototype.concat.call(receiver,huge);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// ERROR_CASES
// case: null receiver
Array.prototype.concat.call(null)
// case: undefined receiver
Array.prototype.concat.call(undefined)
// case: primitive constructor
(function(){var source=[1];source.constructor=1;return source.concat()})()
// case: plain object species
(function(){var source=[1],ctor=Object();ctor[Symbol.species]=Object();source.constructor=ctor;return source.concat()})()
// case: concat itself as species
(function(){var source=[1],ctor=Object();ctor[Symbol.species]=Array.prototype.concat;source.constructor=ctor;return source.concat()})()
// case: Symbol spread length
(function(){var value=Object();value.length=Symbol('length');value[Symbol.isConcatSpreadable]=true;return [].concat(value)})()
// case: BigInt spread length
(function(){var value=Object();value.length=1n;value[Symbol.isConcatSpreadable]=true;return [].concat(value)})()
// case: spreadability getter throw
(function(){var value=Object();value.__defineGetter__(Symbol.isConcatSpreadable,function(){throw 93});return [].concat(value)})()
// case: spread length getter throw
(function(){var value=Object();value[Symbol.isConcatSpreadable]=true;value.__defineGetter__('length',function(){throw 94});return [].concat(value)})()
