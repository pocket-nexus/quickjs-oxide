function __completion(callback){
    try{return "return:"+String(callback())}
    catch(error){
        if(error!==null&&typeof error==="object")
            return "throw:"+error.name+":"+error.message;
        return "throw:"+typeof error+":"+String(error);
    }
}
// GRAMMAR_CASES
// case: literal duplicate add modifier reports the modifier error
/(?ii:a)/
// case: constructor duplicate remove modifier reports the modifier error
(function(){
            return __completion(function(){return new RegExp("(?-ss:a)")});
        })()
// case: add and remove overlap is invalid after both lists are parsed
(function(){
            return __completion(function(){return new RegExp("(?im-ms:a)")});
        })()
// case: empty add and remove lists are invalid
(function(){
            return __completion(function(){return new RegExp("(?-:a)")});
        })()
// case: missing modifier colon wins before parsing the body
(function(){
            return __completion(function(){return new RegExp("(?i=a)")});
        })()
// case: only i m and s enter the scoped modifier grammar
(function(){
            return __completion(function(){return new RegExp("(?g:a)")});
        })()
// case: duplicate add modifier wins before add remove overlap
(function(){
            return __completion(function(){return new RegExp("(?iim-i:a)")});
        })()
// case: unterminated modifier group reports the closing delimiter
(function(){
            return __completion(function(){return new RegExp("(?i:a")});
        })()
// SCOPED_MATCHING_CASES
// case: nested add remove and re-add restores each enclosing modifier state
(function(){
            var regexp=/(?i:a(?-i:b(?i:c))d)e/;
            return [regexp.test("AbCDe"),regexp.test("ABCDe"),
                regexp.test("AbCdE")].join("|");
        })()
// case: ignoreCase scopes over literals and classes then restores outside
(function(){
            var regexp=/(?i:a[A-Z])(?-i:b)/;
            return [regexp.test("aZb"),regexp.test("Azb"),
                regexp.test("AZB")].join("|");
        })()
// case: ignoreCase changes Unicode word boundary classification within scope
(function(){
            var add=/(?i:\b\u212a\b)/u,
                remove=/(?-i:\b\u212a\b)/iu;
            return [add.test("\u212a"),remove.test("\u212a"),
                add.flags,remove.flags].join("|");
        })()
// case: multiline add and remove select scoped anchor semantics
(function(){
            var add=/(?m:^b$)/,
                remove=/(?-m:^b$)/m,
                input="a\nb\nc";
            return [add.test(input),remove.test(input),
                add.multiline,remove.multiline].join("|");
        })()
// case: dotAll add and remove select scoped dot semantics
(function(){
            var add=/(?s:a.b)/,
                remove=/(?-s:a.b)/s,
                input="a\nb";
            return [add.test(input),remove.test(input),
                add.dotAll,remove.dotAll].join("|");
        })()
// CONSTRUCTION_AND_FRONTIER_CASES
// case: scoped modifiers do not change global flags or global exec state
(function(){
            var regexp=/(?i:a)/g,
                first=regexp.exec("AaA"),afterFirst=regexp.lastIndex,
                second=regexp.exec("AaA"),afterSecond=regexp.lastIndex,
                third=regexp.exec("AaA"),afterThird=regexp.lastIndex,
                exhausted=regexp.exec("AaA");
            return [regexp.source,regexp.flags,regexp.global,regexp.ignoreCase,
                first[0],first.index,afterFirst,second[0],second.index,afterSecond,
                third[0],third.index,afterThird,exhausted,regexp.lastIndex].join("|");
        })()
// case: literal and constructor paths preserve equivalent source and flags
(function(){
            var source="(?im-s:^a.$)",
                literal=/(?im-s:^a.$)/s,
                constructed=new RegExp(source,"s"),
                input="x\nA!\ny";
            return [literal.source===constructed.source,literal.flags,
                constructed.flags,literal.test(input),constructed.test(input),
                literal.ignoreCase,literal.multiline,literal.dotAll].join("|");
        })()
// case: scoped dot keeps legacy code-unit and Unicode code-point width
(function(){
            var legacy=/(?s:(.))/.exec("\ud83d\ude00"),
                unicode=/(?s:(.))/u.exec("\ud83d\ude00");
            return [legacy[0].length,legacy[1].length,
                legacy[1].charCodeAt(0).toString(16),
                unicode[0].length,unicode[1].length,
                unicode[1].codePointAt(0).toString(16)].join("|");
        })()
// case: scoped ignoreCase folds astral Unicode without changing outer flags
(function(){
            var add=/(?i:\u{10400})/u,
                remove=/(?-i:\u{10400})/iu,
                lower="\u{10428}";
            return [add.test(lower),remove.test(lower),add.unicode,
                add.ignoreCase,remove.ignoreCase].join("|");
        })()
// case: modifier groups remain noncapturing under alternation and quantification
(function(){
            var regexp=/(?i:(a|b)+)(c)/,
                match=regexp.exec("ABc");
            return [match[0],match[1],match[2],match.length,
                regexp.test("ABC"),regexp.source,regexp.flags].join("|");
        })()
