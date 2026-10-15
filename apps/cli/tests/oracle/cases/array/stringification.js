// VALUE_CASES
// case: dense sparse nullish and nested values stringify without flattening
(function(){
            var nested=[2,null,,4],source=[1,,null,undefined,nested];
            return source.join("|")+"~"+source.toString()+"~"+source.toLocaleString();
        })()
// case: default undefined null empty and multi-code-unit separators stay distinct
(function(){
            var source=[1,2,3];
            return source.join()+"|"+source.join(undefined)+"|"+source.join(null)+"|"+
                source.join("")+"|"+source.join("::");
        })()
// case: join preserves every UTF-16 code unit in elements and separator
(function(){
            function codes(value){
                var result="";
                for(var i=0;i<value.length;i++){
                    if(i)result+=",";
                    result+=value.charCodeAt(i);
                }
                return result;
            }
            var value=["A\uD83D\uDCA9","\uD800","Z"].join("\uDC00\uD83D\uDCA9");
            return value.length+"|"+codes(value);
        })()
// case: toString observes a replaced join on an Array
(function(){
            var log="",source=[1,2];
            source.join=function(argument){log+=(this===source)+":"+(argument===undefined);return "custom"};
            return source.toString()+"|"+log;
        })()
// case: locale stringification covers primitive and nested elements
(function(){
            var symbol=Symbol("s"),nested=[2,3];
            return [1,"x",true,4n,symbol,nested].toLocaleString();
        })()
// case: deep acyclic nesting is not mistaken for recursive overflow
(function(){
            var value="x";
            for(var i=0;i<20;i++)value=[value];
            return value.join();
        })()
// ORDER_AND_GENERIC_CASES
// case: length then separator then indexed Get and conversion have pinned order
(function(){
            var log="",proto=Object(),source=Object.create(proto),separator=Object();
            source.__defineGetter__("length",function(){log+="L";return 3});
            separator.toString=function(){log+="S";return "|"};
            source.__defineGetter__("0",function(){
                var element=Object();log+="G0";
                element.toString=function(){
                    log+="C0";
                    var descriptor=Object();descriptor.value="late";descriptor.configurable=true;
                    Object.defineProperty(source,"2",descriptor);
                    source[3]="ignored";
                    return "a";
                };
                return element;
            });
            proto.__defineGetter__("1",function(){
                var element=Object();log+="G1";
                element.toString=function(){log+="C1";return "p"};
                return element;
            });
            source.__defineGetter__("2",function(){log+="G2";return "old"});
            var result=Array.prototype.join.call(source,separator);
            return result+"|"+log+"|"+(source[3]==="ignored");
        })()
// case: zero length still converts an explicit separator after length
(function(){
            var log="",source=Object(),separator=Object();
            source.__defineGetter__("length",function(){log+="L";return 0});
            separator.toString=function(){log+="S";return "-"};
            return Array.prototype.join.call(source,separator)+"|"+log;
        })()
// case: holes use ordinary Get and therefore see inherited indexed values
(function(){
            var proto=Object(),source=Object.create(proto);
            source.length=4;proto[0]="p0";proto[2]="p2";source[3]="own";
            return Array.prototype.join.call(source,"|");
        })()
// case: String primitives are boxed and traversed as UTF-16 array-likes
(function(){
            function codes(value){
                var result="";
                for(var i=0;i<value.length;i++){
                    if(i)result+=",";
                    result+=value.charCodeAt(i);
                }
                return result;
            }
            var value=Array.prototype.join.call("A\uD83D\uDCA9Z","|");
            return value.length+"|"+codes(value);
        })()
// case: number boolean bigint and symbol receivers have zero array-like length
(function(){
            return Array.prototype.join.call(3)+"|"+
                Array.prototype.join.call(false)+"|"+
                Array.prototype.join.call(4n)+"|"+
                Array.prototype.join.call(Symbol("receiver"));
        })()
// case: fractional negative and NaN lengths use ToLength once
(function(){
            function observed(length){var value=Object();value.length=length;value[0]="a";value[1]="b";return Array.prototype.join.call(value,"|")}
            return observed(2.9)+"~"+observed(-2)+"~"+observed(0/0);
        })()
// TO_STRING_CASES
// case: a callable join result is returned without conversion
(function(){
            var objectResult=Object(),symbolResult=Symbol("result"),log="";
            var first=Object(),second=Object();
            first.join=function(argument){log+=(this===first)+":"+(argument===undefined);return objectResult};
            second.join=function(){return symbolResult};
            return (Array.prototype.toString.call(first)===objectResult)+"|"+
                (Array.prototype.toString.call(second)===symbolResult)+"|"+log;
        })()
// case: a non-callable join uses intrinsic Object toString and toStringTag
(function(){
            var source=Object(),log="",saved=Object.prototype.toString;
            source.join=1;
            source.__defineGetter__(Symbol.toStringTag,function(){log+="T";return "Tagged"});
            Object.prototype.toString=function(){throw 31};
            var result=Array.prototype.toString.call(source);
            Object.prototype.toString=saved;
            return result+"|"+log;
        })()
// case: an Array with a non-callable join uses the intrinsic Array tag
(function(){
            var source=[];source.join=null;
            return Array.prototype.toString.call(source);
        })()
// case: toString boxes a primitive before reading join
(function(){
            var saved=Number.prototype.join,log="";
            Number.prototype.join=function(argument){log+=(typeof this)+":"+this.valueOf()+":"+(argument===undefined);return 17};
            var result=Array.prototype.toString.call(6);
            if(saved===undefined)delete Number.prototype.join;else Number.prototype.join=saved;
            return result+"|"+log;
        })()
// LOCALE_CASES
// case: locale ignores arguments calls methods with zero args and ToStrings results
(function(){
            var log="",element=Object(),ignored=Object();
            ignored.toString=function(){log+="I";throw 41};
            element.__defineGetter__("toLocaleString",function(){
                log+="G";
                return function(argument){
                    log+="M"+(argument===undefined)+":"+(this===element);
                    var result=Object();result.toString=function(){log+="C";return "localized"};
                    return result;
                };
            });
            var result=Array.prototype.toLocaleString.call([element,null,undefined],ignored);
            return result+"|"+log;
        })()
// case: locale snapshots length and observes later indexed mutations
(function(){
            var log="",source=Object();source.length=2;
            var first=Object();source[0]=first;
            first.toLocaleString=function(){
                var second=Object(),third=Object();log+="M0";
                second.toLocaleString=function(){log+="M1";return "late"};
                third.toLocaleString=function(){log+="M2";return "ignored"};
                source[1]=second;source[2]=third;
                source.length=3;return "first";
            };
            return Array.prototype.toLocaleString.call(source)+"|"+log+"|"+source.length;
        })()
// case: locale uses inherited indexed values and Get on holes
(function(){
            var log="",proto=Object(),source=Object.create(proto);source.length=2;
            proto.__defineGetter__("0",function(){
                var element=Object();log+="G";
                element.toLocaleString=function(){log+="M";return "p"};
                return element;
            });
            return Array.prototype.toLocaleString.call(source)+"|"+log;
        })()
// case: locale method null and undefined results are ToStringed rather than skipped
(function(){
            var first=Object(),second=Object();
            first.toLocaleString=function(){return null};
            second.toLocaleString=function(){return undefined};
            return [first,second].toLocaleString();
        })()
// case: locale preserves UTF-16 returned by element conversion
(function(){
            var element=Object();element.toLocaleString=function(){return "\uD800\uD83D\uDCA9"};
            var value=[element,element].toLocaleString(),result=value.length+"";
            for(var i=0;i<value.length;i++)result+="|"+value.charCodeAt(i);
            return result;
        })()
// ERROR_CASES
// case: join null receiver
Array.prototype.join.call(null)
// case: join undefined receiver
Array.prototype.join.call(undefined)
// case: toString null receiver
Array.prototype.toString.call(null)
// case: toLocaleString undefined receiver
Array.prototype.toLocaleString.call(undefined)
// case: Symbol length fails before separator conversion
(function(){
            var source=Object(),separator=Object();source.length=Symbol("length");
            separator.toString=function(){throw 51};
            return Array.prototype.join.call(source,separator);
        })()
// case: Symbol separator cannot be converted
(function(){var source=Object();source.length=0;return Array.prototype.join.call(source,Symbol('separator'))})()
// case: Symbol join element cannot be converted
(function(){var source=Object();source[0]=Symbol('element');source.length=1;return Array.prototype.join.call(source)})()
// case: missing locale method is not callable
(function(){var source=Object();source[0]=Object.create(null);source.length=1;return Array.prototype.toLocaleString.call(source)})()
// case: non-callable locale method is rejected
(function(){var source=Object(),element=Object();element.toLocaleString=1;source[0]=element;source.length=1;return Array.prototype.toLocaleString.call(source)})()
// case: Symbol locale result cannot be converted
(function(){var source=Object(),element=Object();element.toLocaleString=function(){return Symbol('result')};source[0]=element;source.length=1;return Array.prototype.toLocaleString.call(source)})()
// case: indexed getter throw stops before later conversion
(function(){
            var log="",source=Object();source.length=2;
            source.__defineGetter__("0",function(){
                var element=Object();log+="G0";
                element.toString=function(){log+="C0";throw new RangeError("element boom")};
                return element;
            });
            source.__defineGetter__("1",function(){log+="G1";return 2});
            try{Array.prototype.join.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: toString join getter primitive throw is preserved
(function(){
            var source=Object();source.__defineGetter__("join",function(){throw 57});
            return Array.prototype.toString.call(source);
        })()
// case: join is not constructable
new (Array.prototype.join)()
// case: toString is not constructable
new (Array.prototype.toString)()
// case: toLocaleString is not constructable
new (Array.prototype.toLocaleString)()
// CYCLE_CASES
// case: direct join recursion is catchable InternalError stack overflow
(function(){
            var source=[];source[0]=source;
            try{source.join();return "missing"}
            catch(error){return error.name+"|"+error.message}
        })()
// case: direct toString recursion is catchable InternalError stack overflow
(function(){
            var source=[];source[0]=source;
            try{source.toString();return "missing"}
            catch(error){return error.name+"|"+error.message}
        })()
// case: mutual locale recursion is catchable InternalError stack overflow
(function(){
            var first=[],second=[];first[0]=second;second[0]=first;
            try{first.toLocaleString();return "missing"}
            catch(error){return error.name+"|"+error.message}
        })()
