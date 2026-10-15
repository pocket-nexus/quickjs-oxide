function __repeat(value,count){
    var result="";
    while(count-->0)result+=value;
    return result;
}
function __descriptor(value,name){
    var descriptor=Object.getOwnPropertyDescriptor(value,name);
    return [descriptor.writable,descriptor.enumerable,descriptor.configurable].join(",");
}
// GRAMMAR_AND_NAME_CASES
// case: constructor accepts a named capture without Unicode mode
(function(){var r=new RegExp("(?<x>a)");return r.source})()
// case: an empty capture name reports invalid group name
new RegExp("(?<>a)")
// case: a decimal capture-name start reports invalid group name
new RegExp("(?<1>a)")
// case: only Unicode escapes are accepted inside a capture name
new RegExp("(?<\\x61>a)")
// case: an escaped lone surrogate reports invalid group name
new RegExp("(?<\\uD800>a)")
// case: an undefined named backreference reports group name not defined
new RegExp("(?<a>a)\\k<b>")
// case: a bare named-backreference marker reports expecting group name
new RegExp("(?<a>a)\\k")
// case: a malformed named backreference reports invalid group name
new RegExp("(?<a>a)\\k<>")
// case: raw and escaped spellings collide after name decoding
new RegExp("(?<\\u0061>a)(?<a>b)")
// case: duplicate names in one alternative are rejected
new RegExp("(?<x>a)(?<x>b)")
// case: duplicate name wins over the capture-count limit
(function(){var p="(?<x>a)"+__repeat("()",253)+"(?<x>b)";return new RegExp(p)})()
// case: invalid name wins over the capture-count limit
(function(){var p=__repeat("()",254)+"(?<1>a)";return new RegExp(p)})()
// case: QuickJS accepts exactly 254 user capture slots
(function(){var r=new RegExp(__repeat("()",254));return r.source.length>0})()
// case: the 255th user capture reports too many captures
new RegExp(__repeat("()",255))
// case: a 122-byte ASCII group name fits the fixed parser buffer
(function(){var n=__repeat("a",122),r=new RegExp("(?<"+n+">x)");return r.exec("x").groups[n]})()
// case: a 123-byte ASCII group name exceeds the fixed parser buffer
new RegExp("(?<"+__repeat("a",123)+">x)")
// case: invalid flags are diagnosed before invalid group grammar
new RegExp("(?<>a)","uu")
// case: regexp literals report the same duplicate-name diagnostic
/(?<x>a)(?<x>b)/
// case: the wrapping alternative scope collides after 256 bars
(function(){var p="(?<x>a)"+__repeat("|z",255)+"|(?<x>b)";return new RegExp(p)})()
// case: the forward-name scan skips a nested group at body byte zero
new RegExp("\\k<y>(?<x>(?<y>a))","u")
// case: a multibyte final name scalar still obeys the pre-write buffer guard
new RegExp("(?<"+__repeat("a",122)+"𐒤>x)")
// case: the capture prepass stops before a late name after 254 user captures
(function(){var p="\\k<missing>"+__repeat("()",254)+"(?<x>a)";return new RegExp(p)})()
// CAPTURE_AND_BACKREFERENCE_CASES
// case: raw and escaped Unicode names share captures and references
(function(){var m=/(?<π>a)(?<\u{10400}>b)\k<\u03c0>\k<𐐀>/u.exec("abab");return [m[0],m.groups.π,m.groups["𐐀"],Object.keys(m.groups).join(",")].join("|")})()
// case: an escaped surrogate-pair name is canonicalized to its scalar
(function(){var m=/(?<\uD801\uDC00>a)\k<𐐀>/u.exec("aa");return [m[0],m.groups["𐐀"]].join("|")})()
// case: a forward named reference is empty until its capture participates
(function(){var m=/\k<a>(?<a>b)\w\k<a>/u.exec("bab");return [m[0],m[1],m.groups.a].join("|")})()
// case: a named self-reference is empty while entering its capture
(function(){var m=/(?<a>\k<a>\w)../u.exec("bab");return [m[0],m[1],m.groups.a].join("|")})()
// case: Annex B treats k as an identity escape when no names exist
(function(){var r=new RegExp("\\k<x>"),m=r.exec("k<x>");return [r.source,m[0]].join("|")})()
// case: Annex B preserves every malformed k marker when no names exist
(function(){var p=["\\k","\\k<>","\\k<->","\\k<"],out=[],i,r;for(i=0;i<p.length;i++){r=new RegExp(p[i]);out.push(r.source,r.test(p[i].slice(1)))}return out.join("|")})()
// case: duplicate alternatives select the first participating backreference
(function(){var r=/(?:(?<x>a)|(?<x>b))\k<x>/,a=r.exec("aa"),b=r.exec("bb");return [a[0],a[1],String(a[2]),a.groups.x,b[0],String(b[1]),b[2],b.groups.x].join("|")})()
// case: repeated duplicate captures reset stale alternatives
(function(){var m=/(?:(?:(?<x>a)|(?<x>b))\k<x>){2}/.exec("aabb");return [m[0],String(m[1]),m[2],m.groups.x].join("|")})()
// case: QuickJS scope quirk lets sequential duplicates compile after a bar
(function(){var r=/(?<x>a)(?:z|y)(?<x>b)\k<x>/,a=r.exec("azba"),b=r.exec("azbb");return [a[0],a[1],a[2],a.groups.x,b===null].join("|")})()
// case: named captures retain reverse-quantifier semantics in lookbehind
(function(){var a=/(?<=(?<x>\w){3})f/u.exec("abcdef"),b=/(?<=(?<x>\w)+)f/u.exec("abcdef");return [a[0],a.groups.x,b[0],b.groups.x].join("|")})()
// case: a named backreference can execute backward inside lookbehind
(function(){var m=/(?<x>a)b(?<=\k<x>b)c/.exec("zabc");return [m[0],m.groups.x,m.index].join("|")})()
// case: ignoreCase canonicalization applies to named backreferences
(function(){var m=/(?<x>k)\k<x>/iu.exec("kK");return [m[0],m.groups.x].join("|")})()
// case: an unmatched named capture stays undefined in both result views
(function(){var m=/(?<a>a).|(?<x>x)/.exec("ab");return [m[0],m[1],String(m[2]),m.groups.a,String(m.groups.x)].join("|")})()
// GROUPS_AND_INDICES_CASES
// case: groups is null-prototype with ordinary data descriptors
(function(){var m=/(?<__proto__>a)(?<x>b)?/.exec("a");return [Object.getPrototypeOf(m.groups)===null,m.groups.__proto__,String(m.groups.x),__descriptor(m,"groups"),__descriptor(m.groups,"__proto__"),__descriptor(m.groups,"x")].join("|")})()
// case: groups remains an own undefined property without named captures
(function(){var m=/a/.exec("a");return [Object.prototype.hasOwnProperty.call(m,"groups"),String(m.groups),__descriptor(m,"groups")].join("|")})()
// case: every declared name exists even when its alternative is unmatched
(function(){var g=/(?<a>a)|(?<b>b)/.exec("a").groups;return [Object.keys(g).join(","),g.a,String(g.b),Object.prototype.hasOwnProperty.call(g,"b")].join("|")})()
// case: duplicate-name property order follows first source occurrence
(function(){var r=/(?<y>a)(?<x>a)|(?<x>b)(?<y>b)/,a=r.exec("aa").groups,b=r.exec("bb").groups;return [Object.keys(a).join(","),a.y,a.x,Object.keys(b).join(","),b.y,b.x].join("|")})()
// case: a later participating duplicate replaces an earlier undefined value
(function(){var a=/(?<x>a)|(?<x>b)/.exec("b"),b=/(?<x>b)|(?<x>a)/.exec("b");return [a.groups.x,b.groups.x].join("|")})()
// case: indices.groups mirrors names with a null prototype and descriptors
(function(){var m=/(?<a>.)(?<b>.)/d.exec("ab"),g=m.indices.groups;return [Object.getPrototypeOf(g)===null,g.a.join(","),g.b.join(","),__descriptor(m.indices,"groups"),__descriptor(g,"a")].join("|")})()
// case: indices.groups publishes undefined for an unmatched named capture
(function(){var g=/(?<a>a)|(?<b>b)/d.exec("a").indices.groups;return [g.a.join(","),String(g.b),Object.keys(g).join(",")].join("|")})()
// case: duplicate indices select the participating capture
(function(){var r=/(?<x>a)|(?<x>b)/d,a=r.exec("..a"),b=r.exec("..b");return [a.indices.groups.x.join(","),b.indices.groups.x.join(",")].join("|")})()
// case: prototype setters cannot intercept result or group definitions
(function(){var hits=0;Object.defineProperty(Array.prototype,"groups",{set:function(){hits++},configurable:true});Object.defineProperty(Object.prototype,"x",{set:function(){hits++},configurable:true});var m=/(?<x>a)/.exec("a");delete Array.prototype.groups;delete Object.prototype.x;return [hits,m.groups.x].join("|")})()
// REPLACEMENT_CASES
// case: a native named regexp falls back from the direct replacement helper
"abcd".replace(/(?<fst>.)(?<snd>.)/,"$<snd>$<fst>")
// case: global named replacement rebuilds groups for every match
"abcd".replace(/(?<fst>.)(?<snd>.)/g,"$<snd>$<fst>")
// case: missing and unmatched names substitute the empty string
(function(){var r=/(?<a>a).|(?<x>x)/;return ["ab".replace(r,"$<x>"),"ab".replace(r,"$<missing>")].join("|")})()
// case: without named captures dollar-angle remains literal on the direct path
"ab".replace(/(a)/,"$<x>")
// case: an unclosed named replacement token remains literal
"ab".replace(/(?<x>a)/,"$<x")
// case: duplicate names expose the participating value to replacement
(function(){var r=/(?<x>a)|(?<x>b)/g;return "ab".replace(r,"[$<x>][$1][$2]")})()
// case: functional replacement receives the exact groups object last
(function(){var seen;var out="ab".replace(/(?<a>a)(?<b>b)/,function(match,a,b,index,input,groups){seen=[match,a,b,index,input,groups.a,groups.b,Object.getPrototypeOf(groups)===null,arguments.length].join("|");return groups.b+groups.a});return out+";"+seen})()
// case: generic replacement performs a fresh groups Get for every token
(function(){var reads=0,done=false,groups={};Object.defineProperty(groups,"x",{get:function(){reads++;return reads}});var rx={flags:"",exec:function(){if(done)return null;done=true;return {0:"a",length:1,index:0,groups:groups}}};var out=RegExp.prototype[Symbol.replace].call(rx,"a","$<x>$<x>");return [out,reads].join("|")})()
// case: generic replacement reads inherited names and stringifies the value
(function(){var done=false,log="",proto={x:{toString:function(){log+="S";return "X"}}},groups=Object.create(proto),rx={flags:"",exec:function(){if(done)return null;done=true;return {0:"a",length:1,index:0,groups:groups}}};var out=RegExp.prototype[Symbol.replace].call(rx,"a","[$<x>]");return out+"|"+log})()
// case: escaped Unicode capture names are available to dollar-angle replacement
"a".replace(/(?<\u03c0>a)/u,"<$<π>>")
// CONSTRUCTION_AND_COMPILE_CASES
// case: literal and constructor paths publish equivalent named metadata
(function(){var a=/(?<x>a)/,b=new RegExp("(?<x>a)"),ma=a.exec("a"),mb=b.exec("a");return [a.source===b.source,ma.groups.x,mb.groups.x].join("|")})()
// case: RegExp construction from a RegExp preserves named metadata
(function(){var a=/(?<x>a)/d,b=new RegExp(a),m=b.exec("a");return [a!==b,b.source,b.flags,m.groups.x,m.indices.groups.x.join(",")].join("|")})()
// case: explicit constructor flags recompile the copied named source
(function(){var a=/(?<x>a)/,b=new RegExp(a,"id"),m=b.exec("A");return [b.flags,m.groups.x,m.indices.groups.x.join(",")].join("|")})()
// case: RegExp compile replaces unnamed metadata with named metadata
(function(){var r=/old/g;r.lastIndex=9;var result=r.compile("(?<x>a)","d"),m=r.exec("a");return [result===r,r.source,r.flags,r.lastIndex,m.groups.x,m.indices.groups.x.join(",")].join("|")})()
// case: RegExp compile copies named bytecode metadata from its argument
(function(){var source=/(?<x>a)/d,target=/old/g;target.compile(source);var m=target.exec("a");return [target.source,target.flags,m.groups.x,m.indices.groups.x.join(",")].join("|")})()
