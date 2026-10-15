function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+
        __bit(descriptor.configurable);
}
function __completion(callback){
    try{return "return:"+String(callback())}
    catch(error){
        if(error!==null&&typeof error==="object")
            return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
// MATCHING_CASES
// case: dotAll includes only the four ordinary line terminators excluded by dot
(function(){
            var values=["\n","\r","\u2028","\u2029","\v","\f","\u0085","\u180e"];
            function bits(regexp){
                var output="",index=0;
                while(index<values.length)output+=regexp.test(values[index++])?"1":"0";
                return output;
            }
            var astral="\ud83d\ude00",high="\ud83d",low="\ude00",
                legacy=/^.$/s.exec(astral),unicode=/^.$/su.exec(astral),
                unicodeHigh=/^.$/su.exec(high),unicodeLow=/^.$/su.exec(low);
            return [
                bits(/./),bits(/./s),legacy===null,
                unicode[0].length,unicode[0].codePointAt(0).toString(16),
                /^.$/s.test(high),/^.$/s.test(low),
                unicodeHigh[0].charCodeAt(0).toString(16),
                unicodeLow[0].charCodeAt(0).toString(16)
            ].join("|");
        })()
// case: dotAll global and sticky execution retain UTF-16 low-surrogate lastIndex behavior
(function(){
            var input="A\ud83d\ude00B";
            function run(regexp,lastIndex){
                regexp.lastIndex=lastIndex;
                var match=regexp.exec(input);
                if(match===null)return "null:"+regexp.lastIndex;
                return [match.index,match[0].length,
                    match[0].charCodeAt(0).toString(16),regexp.lastIndex].join(":");
            }
            return [
                run(/./suy,2),run(/./sy,2),run(/./sug,2),
                run(/./sg,2),run(/./suy,1)
            ].join("|");
        })()
// SURFACE_AND_CONSTRUCTION_CASES
// case: dotAll getter metadata prototype special case and brands are exact
(function(){
            var descriptor=Object.getOwnPropertyDescriptor(RegExp.prototype,"dotAll"),
                getter=descriptor.get;
            return [
                typeof getter,descriptor.set===undefined,
                descriptor.enumerable,descriptor.configurable,
                getter.name,getter.length,__bits(getter,"name"),__bits(getter,"length"),
                getter.call(/x/s),getter.call(/x/),
                getter.call(RegExp.prototype)===undefined,
                __completion(function(){return getter.call({})}),
                __completion(function(){return getter.call(1)})
            ].join("|");
        })()
// case: constructor and legacy compile preserve replace and canonically order s
(function(){
            var literal=/x/s,copied=new RegExp(literal),replaced=new RegExp(literal,"g"),
                preserved=/old/g,dropped=/old/s,canonical=/old/,
                preserveResult=preserved.compile(literal),
                dropResult=dropped.compile("x","g"),
                canonicalResult=canonical.compile("x","yusmigd");
            return [
                RegExp(literal)===literal,copied!==literal,copied.source,copied.flags,
                replaced.source,replaced.flags,
                preserveResult===preserved,preserved.source,preserved.flags,
                preserved.dotAll,preserved.lastIndex,
                dropResult===dropped,dropped.flags,dropped.dotAll,dropped.lastIndex,
                canonicalResult===canonical,canonical.flags,canonical.dotAll
            ].join("|");
        })()
// SCOPED_MODIFIER_CASES
// case: nested dotAll add remove and re-add restore every enclosing state
(function(){
        var add=/(?s:a.(?-s:b.(?s:c.d)e.)f.)/,
            remove=/(?-s:a.(?s:b.c)d.)e./s;
        return [
            add.test("a\nbXc\ndeXf\n"),
            add.test("a\nb\nc\ndeXf\n"),
            add.test("a\nbXc\nde\nf\n"),
            add.dotAll,add.flags,
            remove.test("aXb\ncdXe\n"),
            remove.test("a\nb\ncdXe\n"),
            remove.dotAll,remove.flags
        ].join("|");
    })()
// SPECIES_CASES
// case: matchAll preserves s while split preserves s and appends sticky
(function(){
        var matching=/./sg,matchHolder=Object(),matchFlags="",
            matchPattern=false,matchArguments=0;
        function MatchSpecies(pattern,flags){
            matchFlags=flags;
            matchPattern=pattern===matching;
            matchArguments=arguments.length;
            return new RegExp(pattern,flags);
        }
        matchHolder[Symbol.species]=MatchSpecies;
        matching.constructor=matchHolder;
        matching.lastIndex=0;
        var iterator=RegExp.prototype[Symbol.matchAll].call(matching,"\n"),
            match=iterator.next().value,done=iterator.next();

        var separator=/./s,splitHolder=Object(),splitFlags="",
            splitPattern=false,splitArguments=0;
        function SplitSpecies(pattern,flags){
            splitFlags=flags;
            splitPattern=pattern===separator;
            splitArguments=arguments.length;
            return new RegExp(pattern,flags);
        }
        splitHolder[Symbol.species]=SplitSpecies;
        separator.constructor=splitHolder;
        separator.lastIndex=7;
        var parts=RegExp.prototype[Symbol.split].call(separator,"\n");

        return [
            matchFlags,matchPattern,matchArguments,matching.lastIndex,
            match.index,match[0].charCodeAt(0),done.done,
            splitFlags,splitPattern,splitArguments,separator.lastIndex,
            parts.length,parts[0],parts[1]
        ].join("|");
    })()
