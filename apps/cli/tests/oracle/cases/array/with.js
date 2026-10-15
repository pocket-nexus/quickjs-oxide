// VALUE_CASES
// case: dense replacement leaves the source unchanged
(function(){
            var source=[10,20,30],result=source.with(1,99);
            return source.length+"|"+source[0]+"|"+source[1]+"|"+source[2]+"|"+
                result.length+"|"+result[0]+"|"+result[1]+"|"+result[2]+"|"+
                Array.isArray(result)+"|"+(Object.getPrototypeOf(result)===Array.prototype);
        })()
// case: holes become dense own undefined properties
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=[,1,,],result=source.with(1,9);
            return source.length+"|"+own(source,0)+"|"+own(source,2)+"|"+
                result.length+"|"+own(result,0)+"|"+(result[0]===undefined)+"|"+
                own(result,1)+"|"+result[1]+"|"+own(result,2)+"|"+(result[2]===undefined);
        })()
// case: inherited values are copied into dense own properties
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            Array.prototype[0]=7;
            var source=Array(3),result=source.with(2,9);
            var observation=result[0]+"|"+(result[1]===undefined)+"|"+result[2]+"|"+
                own(result,0)+"|"+own(result,1)+"|"+own(result,2);
            delete Array.prototype[0];
            return observation;
        })()
// case: omitted replacement writes undefined
(function(){
            var result=[1,2].with(0);
            return result.length+"|"+(result[0]===undefined)+"|"+result[1]+"|"+
                Object.prototype.hasOwnProperty.call(result,0);
        })()
// ORDER_CASES
// case: replacement index is never read
(function(){
            var log="",source=[10,20,30];
            source.__defineGetter__("1",function(){log+="1";throw 71});
            var result=source.with(1,99);
            return result[0]+"|"+result[1]+"|"+result[2]+"|"+log;
        })()
// case: length index and source getters run in pinned order
(function(){
            var log="",length=Object(),index=Object(),source=Object();
            length.valueOf=function(){log+="N";return 4};
            index.valueOf=function(){log+="I";return 2};
            source.__defineGetter__("length",function(){log+="L";return length});
            source.__defineGetter__("0",function(){log+="0";return "a"});
            source.__defineGetter__("1",function(){log+="1";return "b"});
            source.__defineGetter__("2",function(){log+="2";throw 72});
            source.__defineGetter__("3",function(){log+="3";return "d"});
            var result=Array.prototype.with.call(source,index,"X");
            return result[0]+result[1]+result[2]+result[3]+"|"+log;
        })()
// case: constructor and species are not consulted
(function(){
            var log="",speciesSource=[1,2],ctor=Object();
            ctor.__defineGetter__(Symbol.species,function(){log+="s";throw 73});
            speciesSource.constructor=ctor;
            var first=speciesSource.with(0,9);
            var constructorSource=[3,4];
            constructorSource.__defineGetter__("constructor",function(){log+="c";throw 74});
            var second=constructorSource.with(1,8);
            return first[0]+"|"+first[1]+"|"+second[0]+"|"+second[1]+"|"+log;
        })()
// INDEX_CASES
// case: index conversion matrix
(function(){
            var source=[10,20,30];
            return source.with(undefined,9)[0]+"|"+source.with(0/0,9)[0]+"|"+
                source.with(-0,9)[0]+"|"+source.with(1.9,9)[1]+"|"+
                source.with(-1.9,9)[2]+"|"+source.with(-3,9)[0]+"|"+
                source.with("1",9)[1];
        })()
// case: zero length still converts the index before rejecting it
(function(){
            var log="",index=Object();index.valueOf=function(){log+="I";return 0};
            try{Array(0).with(index,9);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: length above the pinned dense limit fails before element access
(function(){
            var log="",source=Object();source.length=2147483648;
            source.__defineGetter__("0",function(){log+="0";throw 75});
            try{Array.prototype.with.call(source,1,9);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// ERROR_CASES
// case: empty receiver index zero
[].with(0,1)
// case: positive out of range index
[1].with(1,2)
// case: negative out of range index
[1].with(-2,2)
// case: positive infinity index
[1].with(1/0,2)
// case: negative infinity index
[1].with(-1/0,2)
// case: Symbol index
[1].with(Symbol('index'),2)
// case: BigInt index
[1].with(0n,2)
// case: null receiver
Array.prototype.with.call(null,0,2)
// GENERIC_CASES
// case: ordinary array-like receiver returns a base Array
(function(){
            var source=Object();source[0]="a";source[2]="c";source.length=3;
            var result=Array.prototype.with.call(source,1,"b");
            return result.length+"|"+result[0]+"|"+result[1]+"|"+result[2]+"|"+
                Array.isArray(result)+"|"+(Object.getPrototypeOf(result)===Array.prototype);
        })()
// case: String receiver copies UTF-16 code units
(function(){
            var source="A\uD83D\uDCA9Z",result=Array.prototype.with.call(source,1,"X");
            return result.length+"|"+result[0]+"|"+result[1]+"|"+
                result[2].charCodeAt(0)+"|"+result[3]+"|"+Array.isArray(result);
        })()
