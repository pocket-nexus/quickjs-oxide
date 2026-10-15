function __bit(value){return value?"1":"0"}
function __bits(object,key){
    var descriptor=Object.getOwnPropertyDescriptor(object,key);
    if(descriptor===undefined)return "missing";
    return __bit(descriptor.writable)+__bit(descriptor.enumerable)+
        __bit(descriptor.configurable);
}
function __units(value){
    var output=[],index=0;
    while(index<value.length){
        output[index]=value.charCodeAt(index).toString(16);
        index++;
    }
    return value.length+"["+output.join(",")+"]";
}
// RESULT_SHAPE_CASES
// case: indices and pair arrays expose own data descriptors including unmatched undefined
(function(){
            var match=/(a)(b)?/d.exec("za"),indices=match.indices,
                pair=indices[0],unmatched=Object.getOwnPropertyDescriptor(indices,"2");
            return [
                Object.getOwnPropertyNames(indices).join(","),
                Object.getOwnPropertyNames(pair).join(","),
                __bits(match,"indices"),__bits(indices,"0"),__bits(indices,"1"),
                __bits(indices,"2"),__bits(indices,"groups"),
                Object.prototype.hasOwnProperty.call(indices,"2"),
                unmatched.value===undefined,
                __bits(pair,"0"),__bits(pair,"1"),__bits(pair,"length"),
                pair[0],pair[1],indices[1][0],indices[1][1],
                Object.getPrototypeOf(indices)===Array.prototype,
                Object.getPrototypeOf(pair)===Array.prototype
            ].join("|");
        })()
// case: duplicate named indices alias the first participating capture pair
(function(){
            var regexp=/(?<x>a)|(?<x>b)/d,
                left=regexp.exec("..a"),right=regexp.exec("..b"),
                leftGroups=left.indices.groups,rightGroups=right.indices.groups;
            return [
                Object.getPrototypeOf(leftGroups)===null,
                Object.getPrototypeOf(rightGroups)===null,
                leftGroups.x===left.indices[1],leftGroups.x===left.indices[2],
                rightGroups.x===right.indices[1],rightGroups.x===right.indices[2],
                Object.prototype.hasOwnProperty.call(left.indices,"2"),
                left.indices[2]===undefined,
                Object.prototype.hasOwnProperty.call(right.indices,"1"),
                right.indices[1]===undefined,
                __bits(leftGroups,"x"),__bits(rightGroups,"x"),
                leftGroups.x.join(","),rightGroups.x.join(",")
            ].join("|");
        })()
// UTF16_LAST_INDEX_CASES
// case: Unicode scanning and sticky execution distinguish a low-surrogate lastIndex
(function(){
        var input="A\ud83d\ude00B";
        function run(regexp,lastIndex){
            regexp.lastIndex=lastIndex;
            var match=regexp.exec(input);
            return [match.index,__units(match[0]),match.indices[0].join(","),
                regexp.lastIndex].join(":");
        }
        return [
            run(/./dgu,2),
            run(/./duy,2),
            run(/./dg,2),
            run(/./du,2),
            run(/./duy,1)
        ].join("|");
    })()
// MATCH_CASES
// case: non-global match preserves indices while global match returns only strings
(function(){
        var single="aba".match(/(a)/d),regexp=/a/dg;
        regexp.lastIndex=2;
        var global="aba".match(regexp);
        return [
            single[0],single[1],single.index,single.input,
            single.indices[0].join(","),single.indices[1].join(","),
            Object.prototype.hasOwnProperty.call(single,"indices"),
            __bits(single,"indices"),
            global.join(","),Object.getOwnPropertyNames(global).join(","),
            Object.prototype.hasOwnProperty.call(global,"indices"),
            Object.prototype.hasOwnProperty.call(global,"index"),
            Object.prototype.hasOwnProperty.call(global,"input"),
            Object.prototype.hasOwnProperty.call(global,"groups"),
            regexp.lastIndex
        ].join("|");
    })()
// SPECIES_FLAGS_CASES
// case: matchAll preserves d and g while split preserves d and appends sticky
(function(){
        var matching=/a/dg,matchHolder=Object(),matchFlags="",
            matchPattern=false,matchArguments=0;
        function MatchSpecies(pattern,flags){
            matchFlags=flags;
            matchPattern=pattern===matching;
            matchArguments=arguments.length;
            return /a/dg;
        }
        matchHolder[Symbol.species]=MatchSpecies;
        matching.constructor=matchHolder;
        matching.lastIndex=1;
        var iterator=RegExp.prototype[Symbol.matchAll].call(matching,"ba"),
            match=iterator.next().value;

        var separator=/,/d,splitHolder=Object(),splitFlags="",
            splitPattern=false,splitArguments=0;
        function SplitSpecies(pattern,flags){
            splitFlags=flags;
            splitPattern=pattern===separator;
            splitArguments=arguments.length;
            return /,/dy;
        }
        splitHolder[Symbol.species]=SplitSpecies;
        separator.constructor=splitHolder;
        separator.lastIndex=7;
        var parts=RegExp.prototype[Symbol.split].call(separator,"a,b");

        return [
            matchFlags,matchPattern,matchArguments,matching.lastIndex,
            match.index,match.indices[0].join(","),
            splitFlags,splitPattern,splitArguments,separator.lastIndex,
            parts.join(",")
        ].join("|");
    })()
// REPLACE_CASES
// case: replacement callbacks neither receive nor read match indices
(function(){
        var nativeSeen="";
        var nativeResult="za".replace(/(?<x>a)/d,
            function(match,capture,index,input,groups){
                nativeSeen=[
                    arguments.length,match,capture,index,input,groups.x,
                    Object.getPrototypeOf(groups)===null,
                    arguments[5]===undefined
                ].join(":");
                return groups.x.toUpperCase();
            });

        var reads=0,calls=0,result=Object(),receiver=Object(),genericSeen="";
        result.length=1;
        result[0]="a";
        result.index=0;
        result.groups=undefined;
        Object.defineProperty(result,"indices",{get:function(){
            reads++;
            throw 73;
        }});
        receiver.flags="";
        receiver.exec=function(){return result};
        var genericResult=RegExp.prototype[Symbol.replace].call(
            receiver,"a",function(match,index,input){
                calls++;
                genericSeen=[arguments.length,match,index,input,
                    arguments[3]===undefined].join(":");
                return "X";
            });
        return [nativeResult,nativeSeen,genericResult,genericSeen,calls,reads].join("|");
    })()
