// prelude: string_capture
// GRAPH_CASES
// case: the four case keys follow valueOf and precede Annex B and constructor
(function(){
            var selected=["trimLeft","toString","valueOf","toLowerCase",
                "toUpperCase","toLocaleLowerCase","toLocaleUpperCase","anchor",
                "big","sup","constructor"];
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
// case: descriptors names lengths function metadata and construct bits are exact
(function(){
            var names=["toLowerCase","toUpperCase","toLocaleLowerCase",
                "toLocaleUpperCase"],output=[],index=0;
            while(index<names.length){
                var name=names[index],fn=String.prototype[name];
                output.push(name+":"+__bits(String.prototype,name)+":"+fn.name+":"+
                    fn.length+":"+Object.getOwnPropertyNames(fn).join(",")+":"+
                    __bits(fn,"length")+":"+__bits(fn,"name")+":"+
                    (Object.getPrototypeOf(fn)===Function.prototype)+":"+
                    (typeof fn)+":"+__isConstructor(fn));
                index++;
            }
            return output.join("|");
        })()
// case: AutoInit identities are stable descriptor backed and mutually distinct
(function(){
            var names=["toLowerCase","toUpperCase","toLocaleLowerCase",
                "toLocaleUpperCase"],functions=[],output=[],index=0,other;
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
                functions[1]===String.prototype.valueOf,
                functions[2]===functions[0],functions[3]===functions[1]);
            return output.join("|");
        })()
// PROPERTY_CASES
// case: lazy toLowerCase can be deleted before materialization
(function(){
            var deleted=delete String.prototype.toLowerCase;
            return [deleted,"toLowerCase" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"toLowerCase"),
                typeof String.prototype.toLowerCase,
                typeof String.prototype.toUpperCase].join("|");
        })()
// case: lazy locale lower assignment becomes an ordinary replacement
(function(){
            String.prototype.toLocaleLowerCase=17;
            return [String.prototype.toLocaleLowerCase,
                __bits(String.prototype,"toLocaleLowerCase"),
                Object.prototype.hasOwnProperty.call(String.prototype,"toLocaleLowerCase"),
                typeof String.prototype.toLocaleUpperCase].join("|");
        })()
// case: materialized methods remain independently deletable and replaceable
(function(){
            var lower=String.prototype.toLowerCase;
            var upper=String.prototype.toUpperCase;
            var localeLower=String.prototype.toLocaleLowerCase;
            var localeUpper=String.prototype.toLocaleUpperCase;
            delete String.prototype.toUpperCase;
            String.prototype.toLocaleLowerCase=23;
            return [typeof lower,"toUpperCase" in String.prototype,
                String.prototype.toLocaleLowerCase,
                String.prototype.toLocaleUpperCase===localeUpper,
                String.prototype.toLowerCase===lower,
                localeLower===lower,localeUpper===upper].join("|");
        })()
// OUTPUT_CASES
// case: ASCII primitives wrappers and ordinary objects convert generically
(function(){return [
            "ABC xyz 123".toLowerCase(),
            "abc XYZ 123".toUpperCase(),
            String.prototype.toLowerCase.call(123),
            String.prototype.toUpperCase.call(true),
            String.prototype.toLowerCase.call(7n),
            String.prototype.toUpperCase.call(Object()),
            String.prototype.toLowerCase.call(new String("Wrap"))
        ].join("|")})()
// case: Unicode simple and multi-code-point mappings use the pinned tables
(function(){return [
            __units("\u0130\u212a\u0178\u1e9e\u03a3".toLowerCase()),
            __units("\u00df\ufb03\u0149\u0587\u0390\u03b0".toUpperCase()),
            __units("\u01f0\u1e96\u1e97\u1e98\u1e99".toUpperCase()),
            __units("\u2126\u212a\u212b".toLowerCase())
        ].join("|")})()
// case: Unicode 17 Beria Erfe astral pairs are not host-Unicode dependent
(function(){return [
            __units(String.fromCodePoint(0x16e40,0x16ea0).toLowerCase()),
            __units(String.fromCodePoint(0x16e60,0x16ebb).toUpperCase()),
            __units(String.fromCodePoint(0x16e5f,0x16eb8).toLowerCase()),
            __units(String.fromCodePoint(0x16e7f,0x16ed3).toUpperCase())
        ].join("|")})()
// case: astral mappings and every unmatched surrogate code unit are preserved
(function(){
            var source="\ud800A\udc00\ud801\udc00\ud801\udc28\udbff\udfff";
            return [source.length,__units(source.toLowerCase()),
                __units(source.toUpperCase()),
                __units("\ud801\udc00".toLowerCase()),
                __units("\ud801\udc28".toUpperCase())].join("|");
        })()
// case: empty and already-cased strings retain exact output
(function(){return [__units("".toLowerCase()),__units("".toUpperCase()),
            __units("already lower \u00df".toLowerCase()),
            __units("ALREADY UPPER \u212a".toUpperCase()),
            __units("\u0000\uffff".toLowerCase()),
            __units("\u0000\uffff".toUpperCase())].join("|")})()
// case: rope conversion crosses the 8192-unit boundary and sigma context leaves
(function(){
            function grow(character,power){
                var value=character,index=0;
                while(index<power){value=value+value;index++}
                return value;
            }
            var source=(grow("A",13)+"\u03a3")+("\u0301B"+grow("Z",13));
            var lower=source.toLowerCase(),upper=source.toUpperCase();
            return [source.length,lower.length,upper.length,
                __units(lower.slice(8188,8200)),__units(upper.slice(8188,8200)),
                __units(lower.slice(0,4)),__units(lower.slice(lower.length-4))].join("|");
        })()
// FINAL_SIGMA_CASES
// case: Greek sigma requires a preceding cased code point and no following cased code point
(function(){
            var values=["\u03a3","\u039f\u03a3","\u039f\u03a3\u0391",
                "A\u03a3","A\u03a3B","A \u03a3","A\u03a3 B"],output=[],index=0;
            while(index<values.length){output.push(__units(values[index].toLowerCase()));index++}
            return output.join("|");
        })()
// case: Case_Ignorable code points are skipped on both sides of sigma
(function(){
            var values=["A\u0301\u03a3","A\u03a3\u0301B","A'\u0301\u03a3",
                "A\u03a3'\u0301B","A\u00ad\u03a3","A\u03a3\u00adB"],output=[],index=0;
            while(index<values.length){output.push(__units(values[index].toLowerCase()));index++}
            return output.join("|");
        })()
// case: surrogates stop context scans while astral cased characters participate
(function(){
            var values=["A\ud800\u03a3","A\u03a3\ud800B",
                "\ud801\udc00\u03a3","A\u03a3\ud801\udc28",
                String.fromCodePoint(0x16ea0)+"\u03a3",
                "A\u03a3"+String.fromCodePoint(0x16ebb)],output=[],index=0;
            while(index<values.length){output.push(__units(values[index].toLowerCase()));index++}
            return output.join("|");
        })()
// LOCALE_CASES
// case: locale-named methods are exact aliases in behavior but not identity
(function(){
            var value="I\u0130i\u00df\u03a3\ud801\udc00";
            return [
                __units(value.toLowerCase()),__units(value.toLocaleLowerCase()),
                __units(value.toLocaleLowerCase("tr")),
                __units(value.toUpperCase()),__units(value.toLocaleUpperCase()),
                __units(value.toLocaleUpperCase("tr")),
                String.prototype.toLowerCase===String.prototype.toLocaleLowerCase,
                String.prototype.toUpperCase===String.prototype.toLocaleUpperCase
            ].join("|");
        })()
// case: all locale arguments including throwing coercion hooks are completely ignored
(function(){
            var hits=0,locale=Object(),extra=Object(),descriptor=Object();
            locale[Symbol.toPrimitive]=function(){hits++;throw "locale primitive"};
            descriptor.get=function(){hits++;throw "locale string"};
            Object.defineProperty(locale,"toString",descriptor);
            extra[Symbol.toPrimitive]=function(){hits++;throw "extra primitive"};
            return [__units("I\u0130".toLocaleLowerCase(locale,extra,Symbol("later"))),
                __units("i\u00df".toLocaleUpperCase(locale,extra,Symbol("later"))),hits].join("|");
        })()
// case: receiver conversion uses a string hint before ignored locale values
(function(){
            var log="",hits=0,receiver=Object(),locale=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "I\u0130"};
            locale[Symbol.toPrimitive]=function(){hits++;throw "locale"};
            var lower=String.prototype.toLocaleLowerCase.call(receiver,locale);
            var upper=String.prototype.toLocaleUpperCase.call(receiver,locale);
            return [__units(lower),__units(upper),log,hits].join("|");
        })()
// case: receiver abrupt completion preserves identity and never touches locale
(function(){
            var sentinel=Object(),receiver=Object(),locale=Object(),hits=0;
            receiver[Symbol.toPrimitive]=function(){throw sentinel};
            locale[Symbol.toPrimitive]=function(){hits++;throw "locale"};
            try{String.prototype.toLocaleLowerCase.call(receiver,locale)}
            catch(error){return [(error===sentinel),hits].join("|")}
            return "missing";
        })()
// ERROR_CASES
// case: toLowerCase rejects a null receiver
String.prototype.toLowerCase.call(null)
// case: toUpperCase rejects an undefined receiver
String.prototype.toUpperCase.call(undefined)
// case: toLocaleLowerCase rejects a Symbol receiver before ignoring locale
String.prototype.toLocaleLowerCase.call(Symbol('receiver'), Symbol('locale'))
// case: toLocaleUpperCase rejects a Symbol receiver
String.prototype.toLocaleUpperCase.call(Symbol('receiver'))
// case: ordinary lower receiver ToPrimitive returning an object is rejected
(function(){
            var receiver=Object();receiver[Symbol.toPrimitive]=function(){return Object()};
            return String.prototype.toLowerCase.call(receiver);
        })()
// case: locale upper receiver ToPrimitive returning an object is rejected
(function(){
            var receiver=Object();receiver[Symbol.toPrimitive]=function(){return Object()};
            return String.prototype.toLocaleUpperCase.call(receiver,"en");
        })()
// CONSTRUCT_CASES
// case: all four case methods reject construction with their own exact name
(function(){
        var names=["toLowerCase","toUpperCase","toLocaleLowerCase","toLocaleUpperCase"];
        var output=[],index=0;
        while(index<names.length){
            var fn=String.prototype[names[index]];
            output.push(names[index]+":"+__capture(function(){return new fn()}));
            index++;
        }
        return output.join("|");
    })()
// STACK_CASES
// case: recursive receiver conversion throws catchably and recovers
(function(){
            var lower=Object(),upper=Object(),lowerError="",upperError="";
            lower[Symbol.toPrimitive]=function(){return String.prototype.toLowerCase.call(lower)};
            upper[Symbol.toPrimitive]=function(){return String.prototype.toUpperCase.call(upper)};
            try{String.prototype.toLowerCase.call(lower)}catch(error){lowerError=error.name+":"+error.message}
            try{String.prototype.toUpperCase.call(upper)}catch(error){upperError=error.name+":"+error.message}
            return [lowerError,upperError,"AbC".toLowerCase(),"AbC".toUpperCase()].join("|");
        })()
// case: case conversion and existing String methods share one recursion guard
(function(){
            var value=Object(),depth=0,errorName="",errorMessage="";
            value[Symbol.toPrimitive]=function(){
                depth++;
                if(depth%8===0)return String.prototype.toLowerCase.call(value);
                if(depth%8===1)return String.prototype.toLocaleUpperCase.call(value,Symbol("ignored"));
                if(depth%8===2)return String.prototype.trim.call(value);
                if(depth%8===3)return "x".padEnd(value,"_");
                if(depth%8===4)return "x".repeat(value);
                if(depth%8===5)return "abcdef".slice(value,4);
                if(depth%8===6)return "abcdef".includes("a",value);
                return String.prototype.bold.call(value);
            };
            try{String.prototype.toUpperCase.call(value)}
            catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"AbC".toLowerCase(),"AbC".toLocaleUpperCase("tr"),
                " x ".trim(),"x".padEnd(3,"_"),"ok".repeat(2),
                "abcdef".slice(1,3),"abcdef".includes("bc"),"x".bold()].join("|");
        })()
