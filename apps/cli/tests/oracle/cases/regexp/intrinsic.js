function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+__bit(descriptor.configurable);
}
function __accessorBits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return (typeof descriptor.get)+":"+(descriptor.set===undefined)+":"+
        __bit(descriptor.enumerable)+__bit(descriptor.configurable);
}
function __isConstructor(value){
    try{Reflect.construct(function(){},[],value);return true}catch(_error){return false}
}
function __error(thunk){
    try{return "return:"+String(thunk())}
    catch(error){
        if(error!==null&&typeof error==="object")return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
function __show(value){
    if(value===undefined)return "undefined";
    if(value===null)return "null";
    if(typeof value==="number"){
        if(value!==value)return "NaN";
        if(value===0)return 1/value===-Infinity?"-0":"+0";
        if(value===Infinity)return "+Infinity";
        if(value===-Infinity)return "-Infinity";
    }
    return String(value);
}
function __callableMetadata(owner,key){
    var fn=owner[key];
    return String(key)+":"+fn.name+":"+fn.length+":"+__isConstructor(fn)+":"+
        __bits(owner,key)+":"+__bits(fn,"name")+":"+__bits(fn,"length");
}
// GRAPH_CASES
// case: RegExp graph descriptors prototype brands and species
(function(){
            var species=Object.getOwnPropertyDescriptor(RegExp,Symbol.species);
            var instance=new RegExp("a","g");
            return [
                "global="+typeof RegExp+":"+__bits(globalThis,"RegExp"),
                "constructor="+RegExp.name+":"+RegExp.length+":"+__isConstructor(RegExp)+":"+
                    __bits(RegExp,"name")+":"+__bits(RegExp,"length")+":"+__bits(RegExp,"prototype"),
                "links="+(Object.getPrototypeOf(RegExp)===Function.prototype)+":"+
                    (Object.getPrototypeOf(RegExp.prototype)===Object.prototype)+":"+
                    (RegExp.prototype.constructor===RegExp)+":"+
                    (Object.getPrototypeOf(instance)===RegExp.prototype),
                "brands="+Object.prototype.toString.call(RegExp.prototype)+":"+
                    Object.prototype.toString.call(instance),
                "prototype="+__bits(RegExp.prototype,"constructor"),
                "species="+(typeof species.get)+":"+(species.set===undefined)+":"+
                    __bit(species.enumerable)+__bit(species.configurable)+":"+
                    species.get.name+":"+species.get.length+":"+__isConstructor(species.get)+":"+
                    (species.get.call(instance)===instance),
                "methods="+__bits(RegExp.prototype,"exec")+":"+
                    __bits(RegExp.prototype,"test")+":"+__bits(RegExp.prototype,"toString"),
                "accessors="+__accessorBits(RegExp.prototype,"source")+":"+
                    __accessorBits(RegExp.prototype,"flags")+":"+
                    __accessorBits(RegExp.prototype,"global")+":"+
                    __accessorBits(RegExp.prototype,"unicodeSets"),
                "lastIndex="+__bits(instance,"lastIndex")+":"+
                    Object.getOwnPropertyNames(instance).join(",")
            ].join("|");
        })()
// case: RegExp native callable names lengths flags and constructor bits
(function(){
            var names=["exec","test","toString"],accessors=["hasIndices","global",
                "ignoreCase","multiline","dotAll","unicode","unicodeSets","sticky",
                "source","flags"],output=["RegExp:"+RegExp.name+":"+RegExp.length+":"+
                __isConstructor(RegExp)],i,descriptor,getter;
            for(i=0;i<names.length;i++)output[output.length]=
                __callableMetadata(RegExp.prototype,names[i]);
            for(i=0;i<accessors.length;i++){
                descriptor=Object.getOwnPropertyDescriptor(RegExp.prototype,accessors[i]);
                getter=descriptor.get;
                output[output.length]=accessors[i]+":"+getter.name+":"+getter.length+":"+
                    __isConstructor(getter)+":"+__bits(getter,"name")+":"+
                    __bits(getter,"length")+":"+__accessorBits(RegExp.prototype,accessors[i]);
            }
            descriptor=Object.getOwnPropertyDescriptor(RegExp,Symbol.species);
            getter=descriptor.get;
            output[output.length]="species:"+getter.name+":"+getter.length+":"+
                __isConstructor(getter)+":"+__bits(getter,"name")+":"+__bits(getter,"length");
            return output.join("|");
        })()
// CONSTRUCTOR_CASES
// case: RegExp call identity construct copy override and empty forms
(function(){
            var original=RegExp("a","g");
            original.lastIndex=7;
            var same=RegExp(original),copy=new RegExp(original),override=new RegExp(original,"i");
            var emptyCall=RegExp(),emptyNew=new RegExp();
            return [same===original,copy!==original,copy.source,copy.flags,copy.lastIndex,
                Object.getPrototypeOf(copy)===RegExp.prototype,override.source,override.flags,
                override.lastIndex,emptyCall.source,emptyCall.flags,emptyNew.source,
                emptyNew.flags,emptyCall!==emptyNew].join("|");
        })()
// case: RegExp identity observes match and constructor while exact branded copy ignores toString
(function(){
            var log="",first=new RegExp("ab","g");
            Object.defineProperty(first,Symbol.match,{configurable:true,get:function(){log+="m";return true}});
            Object.defineProperty(first,"constructor",{configurable:true,get:function(){log+="c";return RegExp}});
            var same=RegExp(first),identityLog=log;
            var second=new RegExp("xy","i"),hit=false;
            second[Symbol.match]=false;
            second.toString=function(){hit=true;throw "must not stringify branded pattern"};
            var callCopy=RegExp(second),newCopy=new RegExp(second);
            return [same===first,identityLog,callCopy!==second,newCopy!==second,
                callCopy.source,callCopy.flags,newCopy.source,newCopy.flags,hit].join("|");
        })()
// case: RegExp constructor conversion and derived prototype observation order
(function(){
            var log="",pattern=Object(),flags=Object(),prototype={marker:42};
            Object.defineProperty(pattern,Symbol.match,{get:function(){log+="m";return false}});
            pattern.toString=function(){log+="p";return "a"};
            flags.toString=function(){log+="f";return "g"};
            var NewTarget=(function(){}).bind(null);
            Object.defineProperty(NewTarget,"prototype",{
                configurable:true,get:function(){log+="n";return prototype}
            });
            var result=Reflect.construct(RegExp,[pattern,flags],NewTarget);
            return [log,result.source,result.flags,result.lastIndex,
                Object.getPrototypeOf(result)===prototype,result.marker].join("|");
        })()
// case: RegExp constructor error classes and left to right abrupt completion
(function(){
            var log="",pattern=Object(),flags=Object(),boom={kind:"boom"};
            pattern.toString=function(){log+="p";return "["};
            flags.toString=function(){log+="f";return "gg"};
            var first=__error(function(){return new RegExp("a","gg")});
            var second=__error(function(){return new RegExp("a","z")});
            var third=__error(function(){return new RegExp("a","uv")});
            var fourth=__error(function(){return new RegExp("[","")});
            var fifth=__error(function(){return new RegExp(pattern,flags)});
            var abruptPattern=Object(),unreached=Object(),caught;
            abruptPattern.toString=function(){log+="x";throw boom};
            unreached.toString=function(){log+="BAD";return "g"};
            try{new RegExp(abruptPattern,unreached)}catch(error){caught=error===boom}
            return [first,second,third,fourth,fifth,log,caught].join("|");
        })()
// case: RegExp constructor exposes pinned compile diagnostics
(function(){
            return [
                __error(function(){return new RegExp(")")}),
                __error(function(){return new RegExp("(?x:a)")}),
                __error(function(){return new RegExp("\\!","u")}),
                __error(function(){return new RegExp("[\\!]","u")}),
                __error(function(){return new RegExp("\\-","u")}),
                __error(function(){return new RegExp("]","u")}),
                __error(function(){return new RegExp("{","u")}),
                __error(function(){return new RegExp("\\c","u")}),
                __error(function(){return new RegExp("\\x","u")}),
                __error(function(){return new RegExp("\\u","u")}),
                __error(function(){return new RegExp("^*")}),
                __error(function(){return new RegExp("^{","u")}),
                __error(function(){return new RegExp("^{1}","u")}),
                __error(function(){return new RegExp("\\b{1}","u")}),
                __error(function(){return new RegExp("a{1","u")}),
                __error(function(){return new RegExp("[\\01]","u")})
            ].join("|");
        })()
// case: RegExp legacy class escape ranges fall back to unions while unicode rejects them
(function(){
            var left=new RegExp("[\\d-a]"),right=new RegExp("[a-\\d]"),both=new RegExp("[\\d-\\w]"),
                identities=new RegExp("[\\8\\9\\k]");
            return [left.test("5"),left.test("-"),left.test("a"),left.test("b"),
                right.test("5"),right.test("-"),right.test("a"),right.test("b"),
                both.test("5"),both.test("-"),both.test("Z"),
                identities.test("8"),identities.test("9"),identities.test("k"),identities.test("7"),
                __error(function(){return new RegExp("[\\d-a]","u")}),
                __error(function(){return new RegExp("[a-\\d]","u")})].join("|");
        })()
// LITERAL_CASES
// case: RegExp literals compile once and allocate a fresh branded value per evaluation
(function(){
            function make(){return /a(b)?/dgi}
            var first=make(),second=make(),descriptor=
                Object.getOwnPropertyDescriptor(first,"lastIndex");
            first.lastIndex=1;
            var match=first.exec("zab");
            return [first!==second,first.source,first.flags,second.lastIndex,
                descriptor.writable,descriptor.enumerable,descriptor.configurable,
                match[0],match[1],match.index,first.lastIndex].join("|");
        })()
// case: RegExp literals bypass the global constructor and mutable constructor prototype
(function(){
            var intrinsic=RegExp.prototype,hits=0;
            RegExp.prototype={replacement:true};
            Object.defineProperty(globalThis,"RegExp",{configurable:true,get:function(){
                hits++;throw Error("observable constructor access");
            }});
            var literal=/a/g;
            return [hits,Object.getPrototypeOf(literal)===intrinsic,literal.source,
                literal.flags,literal.lastIndex].join("|");
        })()
// case: RegExp literal source and flags preserve pinned canonical spelling
(function(){
            var escaped=/a\/b/ims,empty=/(?:)/,newline=/\n/u;
            return [escaped.source,escaped.flags,empty.source,empty.flags,
                newline.source,newline.flags,newline.test("\n")].join("|");
        })()
// ACCESSOR_CASES
// case: RegExp source flag accessors and canonical flags
(function(){
            var value=new RegExp("a/b\nc\r","dgimsuy");
            return [value.source,value.flags,value.hasIndices,value.global,value.ignoreCase,
                value.multiline,value.dotAll,value.unicode,value.unicodeSets,value.sticky,
                RegExp.prototype.source,RegExp.prototype.flags,
                __show(RegExp.prototype.global),__show(RegExp.prototype.unicodeSets),
                RegExp.prototype.toString()].join("|");
        })()
// case: RegExp flags getter is generic and reads d g i m s u v y in order
(function(){
            var receiver=Object(),log="",names=["hasIndices","global","ignoreCase",
                "multiline","dotAll","unicode","unicodeSets","sticky"],letters="dgimsuvy",i;
            for(i=0;i<names.length;i++)(function(name,letter,index){
                Object.defineProperty(receiver,name,{get:function(){log+=letter;return index%2===0}});
            })(names[i],letters.charAt(i),i);
            var getter=Object.getOwnPropertyDescriptor(RegExp.prototype,"flags").get;
            return [getter.call(receiver),log,
                __error(function(){return getter.call(null)})].join("|");
        })()
// case: RegExp source and individual flag getters reject wrong brands after prototype special case
(function(){
            var source=Object.getOwnPropertyDescriptor(RegExp.prototype,"source").get;
            var global=Object.getOwnPropertyDescriptor(RegExp.prototype,"global").get;
            var indices=Object.getOwnPropertyDescriptor(RegExp.prototype,"hasIndices").get;
            return [source.call(RegExp.prototype),__show(global.call(RegExp.prototype)),
                __show(indices.call(RegExp.prototype)),
                __error(function(){return source.call({})}),
                __error(function(){return global.call({})}),
                __error(function(){return indices.call(1)})].join("|");
        })()
// case: RegExp toString is generic and reads source before flags
(function(){
            var receiver=Object(),log="";
            Object.defineProperty(receiver,"source",{get:function(){log+="s";return "body"}});
            Object.defineProperty(receiver,"flags",{get:function(){log+="f";return "gi"}});
            var first=RegExp.prototype.toString.call(receiver);
            return [first,log,
                RegExp.prototype.toString.call({source:17,flags:null}),
                __error(function(){return RegExp.prototype.toString.call(null)})].join("|");
        })()
// EXEC_CASES
// case: RegExp exec publishes captures index input groups and d indices
(function(){
            var match=new RegExp("(a)(b)?","d").exec("za"),indices=match.indices;
            return [match.length,match[0],match[1],__show(match[2]),match.index,match.input,
                __show(match.groups),Object.getPrototypeOf(match)===Array.prototype,
                Object.getOwnPropertyNames(match).join(","),
                __bits(match,"0"),__bits(match,"index"),__bits(match,"input"),
                __bits(match,"groups"),__bits(match,"indices"),
                indices.length,indices[0].join(","),indices[1].join(","),
                __show(indices[2]),__show(indices.groups),
                Object.getPrototypeOf(indices)===Array.prototype,
                Object.getPrototypeOf(indices[0])===Array.prototype].join("|");
        })()
// case: RegExp exec searches leftmost and omits indices without d
(function(){
            var regexp=new RegExp("(a)|(b)"),match=regexp.exec("zzb"),miss=regexp.exec("ccc");
            return [match[0],__show(match[1]),match[2],match.index,match.input,
                match.groups===undefined,("indices" in match),miss===null,
                regexp.lastIndex].join("|");
        })()
// case: RegExp global and sticky exec update and reset lastIndex
(function(){
            var global=new RegExp("a","g"),sticky=new RegExp("a","y"),output=[],match;
            global.lastIndex=1;match=global.exec("baac");
            output[output.length]=match.index+":"+global.lastIndex;
            match=global.exec("baac");output[output.length]=match.index+":"+global.lastIndex;
            match=global.exec("baac");output[output.length]=(match===null)+":"+global.lastIndex;
            sticky.lastIndex=1;match=sticky.exec("baac");
            output[output.length]=match.index+":"+sticky.lastIndex;
            sticky.lastIndex=3;match=sticky.exec("baac");
            output[output.length]=(match===null)+":"+sticky.lastIndex;
            return output.join("|");
        })()
// case: RegExp exec ToString and lastIndex ToLength ordering
(function(){
            var log="",regexp=new RegExp("a"),index=Object(),input=Object();
            index.valueOf=function(){log+="l";return 999};
            input.toString=function(){log+="s";return "ba"};
            regexp.lastIndex=index;
            var match=regexp.exec(input),unchanged=regexp.lastIndex===index;
            var global=new RegExp("a","g"),globalIndex=Object(),globalInput=Object();
            globalIndex.valueOf=function(){log+="L";return 1.9};
            globalInput.toString=function(){log+="S";return "baa"};
            global.lastIndex=globalIndex;
            var globalMatch=global.exec(globalInput);
            return [log,match.index,unchanged,globalMatch.index,global.lastIndex].join("|");
        })()
// case: RegExp exec brand check precedes input conversion
(function(){
            var hit=false,input=Object();
            input.toString=function(){hit=true;throw "must not convert"};
            var result=__error(function(){return RegExp.prototype.exec.call({},input)});
            return [result,hit].join("|");
        })()
// case: RegExp global failure reset observes readonly lastIndex while non-global does not write
(function(){
            var global=new RegExp("a","g"),plain=new RegExp("a");
            global.lastIndex=9;plain.lastIndex=9;
            Object.defineProperty(global,"lastIndex",{writable:false});
            Object.defineProperty(plain,"lastIndex",{writable:false});
            var first=__error(function(){return global.exec("a")});
            var second=plain.exec("a");
            return [first,global.lastIndex,second.index,plain.lastIndex].join("|");
        })()
// case: RegExp global lastIndex clamps negatives and rejects infinity by resetting
(function(){
            var negative=new RegExp("a","g"),fractional=new RegExp("a","g"),
                infinite=new RegExp("a","g"),a,b,c;
            negative.lastIndex=-3;a=negative.exec("ba");
            fractional.lastIndex=1.9;b=fractional.exec("baa");
            infinite.lastIndex=Infinity;c=infinite.exec("baa");
            return [a.index,negative.lastIndex,b.index,fractional.lastIndex,
                c===null,infinite.lastIndex].join("|");
        })()
// TEST_CASES
// case: RegExp test uses custom exec and treats only null as failure
(function(){
            var receiver=Object(),log="",input=Object(),count=0;
            input.toString=function(){log+="s";return "text"};
            Object.defineProperty(receiver,"exec",{get:function(){
                log+="g";
                return function(value){log+="e:"+(this===receiver)+":"+value;count++;return count===1?{}:null};
            }});
            var first=RegExp.prototype.test.call(receiver,input);
            var second=RegExp.prototype.test.call(receiver,input);
            return [first,second,log].join("|");
        })()
// case: RegExp test calls a callable Proxy exec through its apply trap
(function(){
    var log=[];
    var exec=new Proxy(function(value){
        log.push("body:"+value);
        return null;
    },{
        apply:function(target,receiver,argumentsList){
            log.push("apply:"+(receiver.exec===exec));
            return Reflect.apply(target,receiver,argumentsList);
        }
    });
    var receiver={exec:exec};
    return [
        RegExp.prototype.test.call(receiver,"x"),
        log.join(",")
    ].join("|");
})()
// case: RegExp test rejects primitive custom exec results noncallable exec and nullish receivers
(function(){
            var primitive={exec:function(){return 1}},noncallable={exec:17},missing={};
            return [
                __error(function(){return RegExp.prototype.test.call(primitive,"x")}),
                __error(function(){return RegExp.prototype.test.call(noncallable,"x")}),
                __error(function(){return RegExp.prototype.test.call(missing,"x")}),
                __error(function(){return RegExp.prototype.test.call(null,"x")}),
                __error(function(){return RegExp.prototype.test.call(undefined,"x")})
            ].join("|");
        })()
// case: RegExp test gets custom exec before that method observes input and preserves abrupt completion
(function(){
            var log="",receiver=Object(),input=Object(),boom={kind:"boom"},caught;
            input.toString=function(){log+="s";return "x"};
            Object.defineProperty(receiver,"exec",{get:function(){log+="g";throw boom}});
            try{RegExp.prototype.test.call(receiver,input)}catch(error){caught=error===boom}
            return [caught,log].join("|");
        })()
// case: RegExp test honors overridden exec while builtin exec ignores it
(function(){
            var regexp=new RegExp("a"),hit=0;
            regexp.exec=function(value){hit++;return value==="custom"?{}:null};
            var first=regexp.test("custom"),second=regexp.test("a");
            var builtin=RegExp.prototype.exec.call(regexp,"za");
            return [first,second,hit,builtin.index,builtin[0]].join("|");
        })()
// EXEC_REENTRANT_COMPILE_CASES
// case: RegExp exec observes a program compiled while converting its input
(function(){
            var regexp=new RegExp("a"),input=Object();
            input.toString=function(){regexp.compile("b");return "b"};
            var match=regexp.exec(input);
            return [match===null?"null":match[0],regexp.source,regexp.flags,
                regexp.lastIndex].join(":");
        })()
// case: RegExp exec observes a global flag compiled while converting its input
(function(){
            var regexp=new RegExp("a"),input=Object();
            input.toString=function(){regexp.compile("a","g");return "a"};
            var match=regexp.exec(input);
            return [match===null?"null":match[0],regexp.flags,regexp.lastIndex].join(":");
        })()
// case: RegExp exec observes a program compiled while converting lastIndex
(function(){
            var regexp=new RegExp("a"),index=Object();
            index.valueOf=function(){regexp.compile("b");return 0};
            regexp.lastIndex=index;
            var match=regexp.exec("b");
            return [match===null?"null":match[0],regexp.source,regexp.flags,
                regexp.lastIndex].join(":");
        })()
// case: RegExp exec observes a global flag compiled while converting lastIndex
(function(){
            var regexp=new RegExp("a"),index=Object();
            index.valueOf=function(){regexp.compile("a","g");return 0};
            regexp.lastIndex=index;
            var match=regexp.exec("a");
            return [match===null?"null":match[0],regexp.flags,regexp.lastIndex].join(":");
        })()
// case: RegExp exec honors removal of sticky while converting lastIndex
(function(){
            var regexp=new RegExp("a","y"),index=Object();
            index.valueOf=function(){
                regexp.compile("a","");
                regexp.lastIndex=9000;
                return 0;
            };
            regexp.lastIndex=index;
            var match=regexp.exec("a");
            return [match===null?"null":match[0],regexp.flags,regexp.lastIndex].join(":");
        })()
// case: RegExp exec ignores an oversized converted index after sticky is removed
(function(){
            var regexp=new RegExp("a","y"),index=Object();
            index.valueOf=function(){
                regexp.compile("a","");
                regexp.lastIndex=9002;
                return 9001;
            };
            regexp.lastIndex=index;
            var match=regexp.exec("a");
            return [match===null?"null":match[0],regexp.flags,regexp.lastIndex].join(":");
        })()
// case: RegExp exec builds captures groups and indices from the replacement program
(function(){
            var regexp=new RegExp("a","d"),input=Object();
            input.toString=function(){
                regexp.compile("(?<letter>b)","d");
                return "b";
            };
            var match=regexp.exec(input);
            if(match===null)return "null:null";
            var capture=match.length===2&&match[0]==="b"&&match[1]==="b"&&
                match.groups.letter==="b"&&Object.getPrototypeOf(match.groups)===null?
                match.groups.letter:"bad";
            var spans=match.indices.length===2&&match.indices[0].join(",")==="0,1"&&
                match.indices[1].join(",")==="0,1"&&
                match.indices.groups.letter.join(",")==="0,1"&&
                Object.getPrototypeOf(match.indices.groups)===null?
                match.indices.groups.letter.join(","):"bad";
            return capture+":"+spans;
        })()
