// VALUE_CASES
// case: flat defaults to one level and compacts every outer hole
(function(){
            var source=[1,[2,,[3,4]],,5],result=source.flat();
            return result.length+"|"+result[0]+"|"+result[1]+"|"+
                Array.isArray(result[2])+"|"+result[2][0]+"|"+result[2][1]+"|"+
                result[3]+"|"+source.length;
        })()
// case: flat depth zero one two and infinity preserve the pinned nesting
(function(){
            var source=[1,[2,[3,[4]]]],a=source.flat(0),b=source.flat(1);
            var c=source.flat(2),d=source.flat(Infinity);
            return a.length+":"+Array.isArray(a[1])+"|"+
                b.length+":"+Array.isArray(b[2])+"|"+
                c.length+":"+Array.isArray(c[3])+"|"+d.length+":"+d.join(",");
        })()
// case: flat depth uses default undefined and saturating Int32 conversion
(function(){
            function tag(depth){var result=[1,[2,[3]]].flat(depth);
                return result.length+":"+Array.isArray(result[result.length-1])}
            return tag(undefined)+"|"+tag(0/0)+"|"+tag(null)+"|"+tag(1.9)+"|"+
                tag(-1.9)+"|"+tag(Infinity)+"|"+tag(-Infinity)+"|"+tag("2");
        })()
// case: holes compact while inherited values remain visible at every level
(function(){
            Array.prototype[1]="P";
            var inner=Array(3);inner[0]="a";inner[2]=["c"];
            var source=[inner,,["d"]],result=source.flat(2);
            var text=result.length+"|"+result.join(",");
            delete Array.prototype[1];return text;
        })()
// case: only genuine Arrays flatten and generic receivers return a base Array
(function(){
            var spreadable=Object();spreadable[0]="x";spreadable.length=1;
            spreadable[Symbol.isConcatSpreadable]=true;
            var first=[spreadable,[1]].flat();
            var generic=Object();generic[0]=[2];generic[2]=3;generic.length=3;
            generic.__defineGetter__("constructor",function(){throw 71});
            var second=Array.prototype.flat.call(generic);
            var third=Array.prototype.flat.call("ab");
            return (first[0]===spreadable)+"|"+first[1]+"|"+
                second.length+":"+second.join(",")+":"+Array.isArray(second)+"|"+
                third.length+":"+third.join(",");
        })()
// case: flatMap binds thisArg and passes value index and boxed source
(function(){
            var source=[10,,30],receiver=Object(),seen="";receiver.offset=1;
            var result=source.flatMap(function(value,index,object){
                seen+=value+":"+index+":"+(object===source)+":"+(this===receiver)+";";
                return [value+this.offset,index];
            },receiver);
            return result.length+"|"+result.join(",")+"|"+seen;
        })()
// case: flatMap maps only outer present elements and compacts returned holes
(function(){
            var calls=0,result=[1,2].flatMap(function(value){
                calls++;var mapped=Array(3);mapped[0]=value;mapped[2]=[value+10];return mapped;
            });
            return calls+"|"+result.length+"|"+result[0]+"|"+
                Array.isArray(result[1])+":"+result[1][0]+"|"+result[2]+"|"+
                Array.isArray(result[3])+":"+result[3][0];
        })()
// case: flatMap boxes primitive receivers before passing the callback source
(function(){
            var seen="",result=Array.prototype.flatMap.call("ab",function(value,index,source){
                seen+=typeof source+":"+source.length+":"+source[index]+";";return [value];
            });
            return result.length+"|"+result.join(",")+"|"+seen;
        })()
// case: flatMap snapshots outer length but observes later deletion and replacement
(function(){
            var source=[1,2,3],seen="";
            var result=source.flatMap(function(value,index){
                seen+=index;if(index===0){delete source[1];source[2]=9;source[3]=4}
                return [value];
            });
            return result.length+"|"+result.join(",")+"|"+seen+"|"+source.length;
        })()
// ORDER_CASES
// case: generic length depth and indexed Get run in pinned order
(function(){
            var log="",length=Object(),depth=Object(),source=Object();
            length.valueOf=function(){log+="N";return 1};
            depth.valueOf=function(){log+="D";return 0};
            source.__defineGetter__("length",function(){log+="L";return length});
            source.__defineGetter__("0",function(){log+="G";return [1]});
            var result=Array.prototype.flat.call(source,depth);
            return log+"|"+result.length+"|"+Array.isArray(result[0]);
        })()
// case: flat converts depth then creates species before indexed reads
(function(){
            var log="",source=[1,2],ctor=Object(),depth=Object();
            depth.valueOf=function(){log+="D";return 1};
            source.__defineGetter__("0",function(){log+="G";return [1]});
            function Species(){log+="A";source[1]=9;return Object()}
            ctor.__defineGetter__(Symbol.species,function(){log+="S";return Species});
            source.__defineGetter__("constructor",function(){log+="C";return ctor});
            var result=source.flat(depth);
            return log+"|"+result[0]+"|"+result[1]+"|"+("length" in result);
        })()
// case: flatMap rejects its mapper before constructor and species lookup
(function(){
            var log="",source=[1];
            source.__defineGetter__("constructor",function(){log+="C";throw 72});
            try{source.flatMap(1);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: flatMap creates species before alternating source Gets and mapper calls
(function(){
            var log="",source=[1,2],ctor=Object();
            source.__defineGetter__("0",function(){log+="G0";return 1});
            source.__defineGetter__("1",function(){log+="G1";return 2});
            function Species(){log+="A";return Object()}
            ctor.__defineGetter__(Symbol.species,function(){log+="S";return Species});
            source.__defineGetter__("constructor",function(){log+="C";return ctor});
            var result=source.flatMap(function(value,index){log+="M"+index;return [value]});
            return log+"|"+result[0]+result[1]+"|"+("length" in result);
        })()
// case: flatten traverses nested arrays depth first before the next outer index
(function(){
            var log="",nested=[1,2],source=[nested,3];
            nested.__defineGetter__("0",function(){log+="N0";return 1});
            nested.__defineGetter__("1",function(){log+="N1";return 2});
            source.__defineGetter__("0",function(){log+="S0";return nested});
            source.__defineGetter__("1",function(){log+="S1";return 3});
            var result=source.flat();return log+"|"+result.join(",");
        })()
// case: each nested Array length is snapshotted when that frame is entered
(function(){
            var log="",nested=[1,2],source=[nested];
            nested.__defineGetter__("0",function(){log+="G0";nested[2]=3;return 1});
            nested.__defineGetter__("1",function(){log+="G1";nested.length=1;return 2});
            var result=source.flat();return log+"|"+result.length+"|"+result.join(",");
        })()
// SPECIES_CASES
// case: custom species receives zero and CreateDataProperty skips inherited setters
(function(){
            var captured,ctor=Object(),proto=Object(),setterHits=0,called="";
            proto.__defineSetter__("0",function(){setterHits++});
            function Species(length){called+="N"+length;captured=Object.create(proto);return captured}
            ctor[Symbol.species]=Species;var source=[1,[2]];source.constructor=ctor;
            var result=source.flat();
            return (result===captured)+"|"+called+"|"+setterHits+"|"+
                result[0]+result[1]+"|"+("length" in result)+"|"+
                Object.prototype.hasOwnProperty.call(result,"0");
        })()
// case: species may alias the result to the source and later reads see writes
(function(){
            var source=[1,[2,3],4],ctor=Object();function Species(){return source}
            ctor[Symbol.species]=Species;source.constructor=ctor;
            var result=source.flat();
            return (result===source)+"|"+source.length+"|"+source.join(",");
        })()
// case: a rejected custom result definition preserves the completed prefix
(function(){
            var captured,ctor=Object(),descriptor=Object();
            function Species(){
                captured=Object();descriptor.value="fixed";descriptor.writable=false;
                descriptor.enumerable=true;descriptor.configurable=false;
                Object.defineProperty(captured,"1",descriptor);return captured;
            }
            ctor[Symbol.species]=Species;var source=[1,2,3];source.constructor=ctor;
            try{source.flat(0);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+captured[0]+"|"+
                captured[1]+"|"+("length" in captured)}
        })()
// case: generic flatten ignores constructor while using a defining realm base Array
(function(){
            var source=Object();source.length=1;source[0]=[7];
            source.__defineGetter__("constructor",function(){throw 73});
            var result=Array.prototype.flat.call(source);
            return result.length+"|"+result[0]+"|"+Array.isArray(result)+"|"+
                (Object.getPrototypeOf(result)===Array.prototype);
        })()
// ERROR_CASES
// case: flat null receiver
Array.prototype.flat.call(null)
// case: flat undefined receiver
Array.prototype.flat.call(undefined)
// case: flat Symbol depth
[1].flat(Symbol('depth'))
// case: flat BigInt depth
[1].flat(1n)
// case: flatMap omitted mapper
[1].flatMap()
// case: flatMap null mapper
[1].flatMap(null)
// case: flatMap ordinary object mapper
[1].flatMap(Object())
// case: flatMap preserves primitive mapper throw
[1].flatMap(function(){throw 77})
// case: flat preserves source getter throw
(function(){var source=[1];source.__defineGetter__("0",function(){throw 78});
            return source.flat()})()
// case: flat preserves species getter throw
(function(){var source=[1],ctor=Object();
            ctor.__defineGetter__(Symbol.species,function(){throw 79});source.constructor=ctor;
            return source.flat()})()
// RECURSION_CASES
// case: four nested mapper reentries complete before the local safety ceiling
(function(){
            function nest(count){
                return [1].flatMap(function(value){return count?nest(count-1):[value]})
            }
            return nest(4)[0];
        })()
// case: self-referential infinity flatten throws a catchable stack overflow
(function(){var source=[];source[0]=source;
            try{source.flat(Infinity);return "missing"}
            catch(error){return error.name+"|"+error.message}})()
// case: deep nesting straddles the pinned stack guard
(function(){
            function run(count){var source=[1];
                for(var index=0;index<count;index++)source=[source];
                try{var result=source.flat(Infinity);return "ok:"+result[0]}
                catch(error){return error.name+":"+error.message}}
            return run(1000)+"|"+run(10000);
        })()
// case: recursive indexed getters stay catchable at the native-call boundary
(function(){var source=[1];
            source.__defineGetter__("0",function(){return source.flat(Infinity)});
            try{source.flat(Infinity);return "missing"}
            catch(error){return error.name+"|"+error.message}})()
