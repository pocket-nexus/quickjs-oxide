// prelude: string_capture
// GRAPH_CASES
// case: thirteen String keys precede constructor while the iterator Symbol remains last
(function(){
            var selected=["trimLeft","toString","valueOf","anchor","big","blink",
                "bold","fixed","fontcolor","fontsize","italics","link","small",
                "strike","sub","sup","constructor"];
            var keys=Object.getOwnPropertyNames(String.prototype)
                .concat(Object.getOwnPropertySymbols(String.prototype));
            var output=[],index=0;
            while(index<keys.length){
                var key=keys[index];
                if(key===Symbol.iterator)output.push("@@iterator");
                else if(selected.indexOf(key)>=0)output.push(key);
                index++;
            }
            return output.join(",");
        })()
// case: all descriptors names lengths GenericMagic-facing metadata and construct bits are exact
(function(){
            var names=["anchor","big","blink","bold","fixed","fontcolor","fontsize",
                "italics","link","small","strike","sub","sup"];
            var lengths=[1,0,0,0,0,1,1,0,1,0,0,0,0],output=[],index=0;
            while(index<names.length){
                var name=names[index],fn=String.prototype[name];
                output.push(name+":"+__bits(String.prototype,name)+":"+fn.name+":"+
                    fn.length+":"+(fn.length===lengths[index])+":"+
                    Object.getOwnPropertyNames(fn).join(",")+":"+
                    __bits(fn,"length")+":"+__bits(fn,"name")+":"+
                    (Object.getPrototypeOf(fn)===Function.prototype)+":"+
                    (typeof fn)+":"+__isConstructor(fn));
                index++;
            }
            return output.join("|");
        })()
// case: AutoInit identities are stable descriptor-backed and mutually distinct
(function(){
            var names=["anchor","big","blink","bold","fixed","fontcolor","fontsize",
                "italics","link","small","strike","sub","sup"];
            var functions=[],output=[],index=0,other;
            while(index<names.length){
                var name=names[index],first=String.prototype[name];
                functions.push(first);
                output.push(first===String.prototype[name]);
                output.push(first===Object.getOwnPropertyDescriptor(String.prototype,name).value);
                index++;
            }
            var distinct=true;
            index=0;
            while(index<functions.length){
                other=index+1;
                while(other<functions.length){
                    if(functions[index]===functions[other])distinct=false;
                    other++;
                }
                index++;
            }
            output.push(distinct,functions[0]===String.prototype.trim,
                functions[12]===String.prototype.valueOf);
            return output.join("|");
        })()
// PROPERTY_CASES
// case: lazy anchor can be deleted before materialization
(function(){
            var deleted=delete String.prototype.anchor;
            return [deleted,"anchor" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"anchor"),
                typeof String.prototype.anchor].join("|");
        })()
// case: lazy bold assignment becomes an ordinary replacement
(function(){
            String.prototype.bold=17;
            return [String.prototype.bold,__bits(String.prototype,"bold"),
                Object.prototype.hasOwnProperty.call(String.prototype,"bold"),
                typeof String.prototype.big].join("|");
        })()
// case: a materialized link remains deletable without affecting its peers
(function(){
            var link=String.prototype.link,anchor=String.prototype.anchor;
            var deleted=delete String.prototype.link;
            return [typeof link,deleted,"link" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"link"),
                String.prototype.anchor===anchor,typeof String.prototype.fontcolor].join("|");
        })()
// case: overwriting multiple properties never aliases another selector
(function(){
            var big=String.prototype.big,sup=String.prototype.sup;
            String.prototype.anchor=23;
            String.prototype.sub=29;
            var before=[String.prototype.big===big,String.prototype.sup===sup,
                String.prototype.anchor,String.prototype.sub];
            delete String.prototype.big;
            return before.concat(["big" in String.prototype,
                String.prototype.sup===sup]).join("|");
        })()
// OUTPUT_CASES
// case: all thirteen selectors use their exact tag and attribute mapping
(function(){
            var source="A&B";
            return [source.anchor("spot"),source.big(),source.blink(),source.bold(),
                source.fixed(),source.fontcolor("red"),source.fontsize(7),source.italics(),
                source.link("url"),source.small(),source.strike(),source.sub(),source.sup()].join("|");
        })()
// case: empty source and empty attributes retain exact opening and closing markup
(function(){return [
            "".anchor(""),"".fontcolor(""),"".fontsize(""),"".link(""),
            "".big(),"".bold(),"".fixed(),"".italics(),"".sup()
        ].join("|")})()
// case: generic primitive wrapper and ordinary object receivers convert
(function(){return [
            String.prototype.big.call(123),
            String.prototype.bold.call(true),
            String.prototype.fixed.call(7n),
            String.prototype.italics.call(Object()),
            String.prototype.sup.call(new String("xy")),
            String.prototype.link.call(42,"target")
        ].join("|")})()
// case: attribute primitives and object-produced null or undefined stringify after the direct check
(function(){
            var nullResult=Object(),undefinedResult=Object();
            nullResult[Symbol.toPrimitive]=function(){return null};
            undefinedResult[Symbol.toPrimitive]=function(){return undefined};
            return ["x".anchor(0),"x".fontcolor(false),"x".fontsize(7n),
                "x".link(Object()),"x".anchor(nullResult),
                "x".link(undefinedResult)].join("|");
        })()
// UTF16_ESCAPE_CASES
// case: only double quotes are escaped in attributes
(function(){
            var attribute="a\"b&<>'\u0000Z",source="\"&<>source";
            return [__units(source.anchor(attribute)),__units(source.fontcolor(attribute)),
                __units(source.fontsize("\"\"\"")),__units(source.link("&<>"))].join("|");
        })()
// case: source and attribute preserve astral and lone surrogate code units
(function(){
            var source="A\ud800\ud83d\ude00\udc00\u0000Z";
            var attribute="Q\udc00\ud83d\ude00\ud800\"R";
            return [source.length,attribute.length,__units(source.anchor(attribute)),
                __units(source.big()),__units(source.link(attribute))].join("|");
        })()
// case: rope source and attribute cross 8192-unit and quote-expansion boundaries
(function(){
            function grow(character,power){
                var value=character,index=0;
                while(index<power){value=value+value;index++}
                return value;
            }
            var source=(grow("s",13)+"\ud800")+("\udc00"+grow("t",13));
            var attribute=(grow("q",13)+"\"")+("\ud83d\ude00"+grow("r",13));
            var result=source.link(attribute),openEnd=result.indexOf(">")+1;
            return [source.length,attribute.length,result.length,openEnd,
                __units(result.slice(0,10)),__units(result.slice(8187,8205)),
                __units(result.slice(openEnd-12,openEnd+8)),
                __units(result.slice(openEnd+source.length-5))].join("|");
        })()
// ORDER_CASES
// case: attribute variants convert receiver then argv0 with string hints and ignore extras
(function(){
            function run(name){
                var log="",receiver=Object(),attribute=Object(),extra=Object();
                receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "R"};
                attribute[Symbol.toPrimitive]=function(hint){log+="attribute:"+hint+";";return "A\"B"};
                extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";throw "extra"};
                return String.prototype[name].call(receiver,attribute,extra,Symbol("later"))+":"+log;
            }
            return [run("anchor"),run("fontcolor"),run("fontsize"),run("link")].join("|");
        })()
// case: ordinary ToPrimitive fallback uses toString before valueOf for both values
(function(){
            var log="",receiver=Object(),attribute=Object();
            receiver.toString=function(){log+="receiver:toString;";return "R"};
            receiver.valueOf=function(){log+="receiver:valueOf;";throw "wrong receiver order"};
            attribute.toString=function(){log+="attribute:toString;";return "A"};
            attribute.valueOf=function(){log+="attribute:valueOf;";throw "wrong attribute order"};
            return String.prototype.link.call(receiver,attribute)+"|"+log;
        })()
// case: receiver abrupt completion preserves identity and prevents attribute conversion
(function(){
            var sentinel=Object(),receiver=Object(),attribute=Object(),log="";
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";throw sentinel};
            attribute[Symbol.toPrimitive]=function(hint){log+="attribute:"+hint+";";return "A"};
            try{String.prototype.anchor.call(receiver,attribute)}
            catch(error){return [(error===sentinel),log].join("|")}
            return "missing";
        })()
// case: attribute abrupt completion follows receiver and preserves identity
(function(){
            var sentinel=Object(),receiver=Object(),attribute=Object(),extra=Object(),log="";
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "R"};
            attribute[Symbol.toPrimitive]=function(hint){log+="attribute:"+hint+";";throw sentinel};
            extra[Symbol.toPrimitive]=function(hint){log+="extra:"+hint+";";return "X"};
            try{String.prototype.fontcolor.call(receiver,attribute,extra)}
            catch(error){return [(error===sentinel),log].join("|")}
            return "missing";
        })()
// case: all nine no-attribute variants ignore every supplied argument
(function(){
            var names=["big","blink","bold","fixed","italics","small","strike","sub","sup"];
            var extra=Object(),hits=0,output=[],index=0;
            extra[Symbol.toPrimitive]=function(hint){hits++;throw "extra"};
            while(index<names.length){
                output.push(String.prototype[names[index]].call("R",extra,Symbol("later"),1n));
                index++;
            }
            output.push(hits);
            return output.join("|");
        })()
// ERROR_CASES
// case: no-attribute method rejects null receiver
String.prototype.bold.call(null)
// case: attribute method rejects undefined receiver before its missing attribute
String.prototype.anchor.call(undefined)
// case: no-attribute method rejects Symbol receiver
String.prototype.sup.call(Symbol('receiver'))
// case: anchor rejects a missing attribute
String.prototype.anchor.call('x')
// case: link rejects an explicit undefined attribute
'x'.link(undefined)
// case: fontcolor rejects a null attribute
'x'.fontcolor(null)
// case: fontsize rejects a Symbol attribute
'x'.fontsize(Symbol('attribute'))
// case: receiver ToPrimitive returning an object is rejected
(function(){
            var receiver=Object();receiver[Symbol.toPrimitive]=function(){return Object()};
            return String.prototype.big.call(receiver);
        })()
// case: attribute ToPrimitive returning an object is rejected
(function(){
            var attribute=Object();attribute[Symbol.toPrimitive]=function(){return Object()};
            return "x".anchor(attribute);
        })()
// CONSTRUCT_CASES
// case: all thirteen methods reject construction with their own exact name
(function(){
        var names=["anchor","big","blink","bold","fixed","fontcolor","fontsize",
            "italics","link","small","strike","sub","sup"];
        var output=[],index=0;
        while(index<names.length){
            var fn=String.prototype[names[index]];
            output.push(names[index]+":"+__capture(function(){return new fn()}));
            index++;
        }
        return output.join("|");
    })()
// STACK_CASES
// case: recursive receiver and attribute conversions throw catchably and recover
(function(){
            var receiver=Object(),attribute=Object(),receiverError="",attributeError="";
            receiver[Symbol.toPrimitive]=function(){return String.prototype.bold.call(receiver)};
            try{String.prototype.bold.call(receiver)}catch(error){receiverError=error.name+":"+error.message}
            attribute[Symbol.toPrimitive]=function(){return "x".anchor(attribute)};
            try{"x".anchor(attribute)}catch(error){attributeError=error.name+":"+error.message}
            return [receiverError,attributeError,"x".bold(),"x".anchor("n")].join("|");
        })()
// case: CreateHTML and existing String methods share one recursion guard
(function(){
            var value=Object(),depth=0,errorName="",errorMessage="";
            value[Symbol.toPrimitive]=function(){
                depth++;
                if(depth%7===0)return String.prototype.bold.call(value);
                if(depth%7===1)return "x".anchor(value);
                if(depth%7===2)return String.prototype.trim.call(value);
                if(depth%7===3)return "x".padEnd(value,"_");
                if(depth%7===4)return "x".repeat(value);
                if(depth%7===5)return "abcdef".slice(value,4);
                return "abcdef".includes("a",value);
            };
            try{String.prototype.big.call(value)}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"x".big(),"x".link("u")," x ".trim(),
                "x".padEnd(3,"_"),"ok".repeat(2),"abcdef".slice(1,3),
                "abcdef".includes("bc")].join("|");
        })()
