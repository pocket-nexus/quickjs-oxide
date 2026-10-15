// SAME_VALUE_CASES
// case: missing arguments nullish booleans and type boundaries
(function(){
            return [
                Object.is(),Object.is(undefined),Object.is(undefined,undefined),
                Object.is(null,null),Object.is(false,false),Object.is(true,true),
                Object.is(null,undefined),Object.is(false,0),Object.is(true,1),
                Object.is("1",1),Object.is(1,1n),Object.is(1,1,2),Object.is(1,2,1)
            ].join(":");
        })()
// case: SameValue distinguishes signed zero and equates every NaN
(function(){
            var positive=0,negative=-0;
            return [
                Object.is(positive,positive),Object.is(negative,negative),
                Object.is(positive,negative),Object.is(-negative,positive),
                Object.is(NaN,NaN),Object.is(0/0,Number("not-a-number")),
                Object.is(Infinity,1/0),Object.is(-Infinity,-1/0),
                Object.is(Infinity,-Infinity),Object.is(1,1.0),Object.is(1,1.0000000000000002)
            ].join(":");
        })()
// case: strings BigInts and Symbols use primitive value or identity semantics
(function(){
            var prefix="ox",shared=Symbol("shared"),other=Symbol("shared");
            var registeredA=Symbol.for("quickjs-oxide-object-is");
            var registeredB=Symbol.for("quickjs-oxide-object-is");
            var large=900719925474099312345678901234567890n;
            return [
                Object.is("oxide",prefix+"ide"),Object.is("oxide","Oxide"),
                Object.is("",""+""),Object.is(0n,-0n),Object.is(7n,3n+4n),
                Object.is(large,900719925474099312345678901234567890n),
                Object.is(large,large+1n),Object.is(shared,shared),Object.is(shared,other),
                Object.is(registeredA,registeredB),Object.is(Symbol.iterator,Symbol.iterator)
            ].join(":");
        })()
// case: ordinary exotic callable and boxed object values compare only by identity
(function(){
            var object=Object(),other=Object(),array=[],fn=function(){},error=new Error("x");
            var boxedNumber=Object(1),boxedString=Object("x"),boxedBigInt=Object(1n);
            return [
                Object.is(object,object),Object.is(object,other),Object.is(array,array),
                Object.is([],[]),Object.is(fn,fn),Object.is(fn,function(){}),
                Object.is(error,error),Object.is(new Error("x"),new Error("x")),
                Object.is(boxedNumber,boxedNumber),Object.is(boxedNumber,Object(1)),
                Object.is(boxedNumber,1),Object.is(boxedString,"x"),Object.is(boxedBigInt,1n)
            ].join(":");
        })()
// NO_COERCION_CASES
// case: comparison does not read or call primitive conversion hooks
(function(){
            var log="",left=Object(),right=Object();
            left[Symbol.toPrimitive]=function(){log+="p";throw "primitive"};
            left.valueOf=function(){log+="v";throw "valueOf"};
            left.toString=function(){log+="s";throw "toString"};
            right[Symbol.toPrimitive]=left[Symbol.toPrimitive];
            return [
                Object.is(left,left),Object.is(left,right),Object.is(left,1),
                Object.is(1,left),Object.is.call(right,left,left),log
            ].join(":");
        })()
// case: comparison does not inspect wrappers getters or object structure
(function(){
            var calls=0,primitiveSymbol=Symbol("x"),number=Object(3),string=Object("ab"),symbol=Object(primitiveSymbol);
            number.__defineGetter__("valueOf",function(){calls++;throw 1});
            string.__defineGetter__("toString",function(){calls++;throw 2});
            symbol.__defineGetter__(Symbol.toPrimitive,function(){calls++;throw 3});
            var sameNumber=number,sameString=string,sameSymbol=symbol;
            return [
                Object.is(number,sameNumber),Object.is(string,sameString),
                Object.is(symbol,sameSymbol),Object.is(number,3),Object.is(string,"ab"),
                Object.is(symbol,primitiveSymbol),calls
            ].join(":");
        })()
// case: this value is ignored and frozen aliases preserve identity
(function(){
            var receiver=Object(),value=Object(),alias=value;
            Object.preventExtensions(value);
            return [
                Object.is.call(receiver,value,alias),Object.is.call(null,+0,-0),
                Object.is.call(undefined,NaN,NaN),Object.is.apply(receiver,[value,value]),
                Object.is.apply(receiver,[value,Object()])
            ].join(":");
        })()
