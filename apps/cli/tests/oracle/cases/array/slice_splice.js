// VALUE_CASES
// case: slice clamps positive and negative bounds without changing its source
(function(){
            var source=[0,1,2,3,4],all=source.slice(),middle=source.slice(1,-1);
            var tail=source.slice(-2,99),empty=source.slice(4,1);
            return all.join("")+"|"+middle.join("")+"|"+tail.join("")+"|"+
                empty.length+"|"+source.join("");
        })()
// case: slice and splice preserve holes while copying inherited values as own data
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            Array.prototype[1]="proto";
            var first=Array(4);first[0]="a";first[3]="d";
            var sliced=first.slice(0,4);
            var second=Array(4);second[0]="a";second[3]="d";
            var removed=second.splice(0,3);
            var result=sliced.length+"|"+sliced[0]+"|"+sliced[1]+"|"+own(sliced,1)+"|"+
                own(sliced,2)+"|"+removed.length+"|"+removed[1]+"|"+own(removed,1)+"|"+
                own(removed,2)+"|"+second.length+"|"+second[0];
            delete Array.prototype[1];return result;
        })()
// case: splice shrink grow and equal replacement return deleted values and mutate in place
(function(){
            var shrink=[0,1,2,3,4],a=shrink.splice(1,3,"x");
            var grow=[0,1,2,3],b=grow.splice(1,1,"x","y","z");
            var equal=[0,1,2],c=equal.splice(1,1,"x");
            return a.join(",")+":"+shrink.join(",")+"|"+
                b.join(",")+":"+grow.join(",")+"|"+
                c.join(",")+":"+equal.join(",");
        })()
// case: splice actual argc distinguishes omitted start and deleteCount
(function(){
            var a=[1,2],r0=a.splice();
            var b=[1,2],r1=b.splice(undefined);
            var c=[1,2],r2=c.splice(1);
            var d=[1,2],r3=d.splice(1,undefined,9);
            return r0.length+":"+a.join(",")+"|"+r1.join(",")+":"+b.length+"|"+
                r2.join(",")+":"+c.join(",")+"|"+r3.length+":"+d.join(",");
        })()
// case: toSpliced actual argc mirrors the pinned omitted deleteCount branches
(function(){
            var source=[1,2],a=source.toSpliced(),b=source.toSpliced(undefined);
            var c=source.toSpliced(1),d=source.toSpliced(1,undefined,9);
            return a.join(",")+"|"+b.length+"|"+c.join(",")+"|"+d.join(",")+"|"+
                source.join(",");
        })()
// case: toSpliced densifies holes and inherited values without changing its source
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            Array.prototype[2]="proto";
            var source=Array(5);source[0]="a";source[4]="e";
            var result=source.toSpliced(1,1,"x",undefined);
            var text=result.length+"|"+result[0]+"|"+(result[1]===undefined)+"|"+
                result[2]+"|"+result[3]+"|"+(result[4]===undefined)+"|"+result[5]+"|"+
                own(result,0)+own(result,1)+own(result,2)+own(result,3)+own(result,4)+own(result,5)+"|"+
                source.length+"|"+own(source,1)+own(source,2);
            delete Array.prototype[2];return text;
        })()
// case: all three methods preserve object identity for copied and inserted values
(function(){
            var a=Object(),b=Object(),source=[a,b];
            var sliced=source.slice(0,1),removed=source.splice(1,1,a);
            var copied=source.toSpliced(1,1,b);
            return (sliced[0]===a)+"|"+(removed[0]===b)+"|"+(source[1]===a)+"|"+
                (copied[0]===a)+"|"+(copied[1]===b);
        })()
// case: NaN infinities fractions strings null and undefined use pinned bound conversion
(function(){
            var source=[0,1,2,3];
            var a=source.slice(NaN,Infinity),b=source.slice(-Infinity,2.9);
            var c=source.slice("2",-1),d=source.slice(1,undefined),e=source.slice(1,null);
            var f=source.toSpliced(-2.9,Infinity,"x");
            return a.join("")+"|"+b.join("")+"|"+c.join("")+"|"+d.join("")+"|"+
                e.length+"|"+f.join(",");
        })()
// ORDER_AND_SPECIES_CASES
// case: slice converts bounds before constructor species and indexed reads
(function(){
            var log="",source=[0,1,2,3],ctor=Object(),start=Object(),end=Object();
            start.valueOf=function(){log+="S";return 1};
            end.valueOf=function(){log+="E";return 3};
            function Species(length){
                log+="A"+length;var result=Object();
                result.__defineSetter__("length",function(value){log+="F"+value});return result;
            }
            ctor.__defineGetter__(Symbol.species,function(){log+="P";return Species});
            source.__defineGetter__("constructor",function(){log+="C";return ctor});
            source.__defineGetter__("1",function(){log+="G1";return "b"});
            source.__defineGetter__("2",function(){log+="G2";return "c"});
            var result=source.slice(start,end);
            return log+"|"+result[0]+result[1]+"|"+("length" in result);
        })()
// case: splice finishes the removed result before moving and inserting on source
(function(){
            var log="",source=[0,1,2],ctor=Object(),one,two;
            function Species(length){
                log+="N"+length;var result=Object();
                result.__defineSetter__("length",function(value){log+="L"+value});return result;
            }
            ctor[Symbol.species]=Species;source.constructor=ctor;
            source.__defineGetter__("1",function(){log+="g1";return 1});
            source.__defineSetter__("1",function(value){log+="s1"+value;one=value});
            source.__defineGetter__("2",function(){log+="g2";return 2});
            source.__defineSetter__("2",function(value){log+="s2"+value;two=value});
            var result=source.splice(1,1,"x","y");
            return log+"|"+result[0]+"|"+one+"|"+two+"|"+source.length;
        })()
// case: species result uses CreateDataProperty and leaves preseeded hole properties intact
(function(){
            var setterHits=0,lengthLog="",proto=Object(),ctor=Object();
            proto.__defineSetter__("0",function(){setterHits++});
            proto.__defineSetter__("length",function(value){lengthLog+="L"+value});
            function Species(){var result=Object.create(proto);result[1]="seed";return result}
            ctor[Symbol.species]=Species;
            var source=[4,,6];source.constructor=ctor;var result=source.slice();
            return result[0]+"|"+result[1]+"|"+result[2]+"|"+setterHits+"|"+lengthLog+"|"+
                Object.prototype.hasOwnProperty.call(result,"0");
        })()
// case: generic slice ignores a throwing constructor while custom species sees exact empty count
(function(){
            var generic=Object(),log="";generic.length=1;generic[0]="x";
            generic.__defineGetter__("constructor",function(){log+="bad";throw 71});
            var first=Array.prototype.slice.call(generic);
            var source=[1,2],ctor=Object();
            function Species(length){log+="N"+length;return Object()}
            ctor[Symbol.species]=Species;source.constructor=ctor;
            var second=source.slice(2,1);
            return first[0]+"|"+first.length+"|"+log+"|"+second.length;
        })()
// case: a failed removed-result length Set prevents splice source mutation
(function(){
            var captured,ctor=Object(),source=[1,2,3],descriptor=Object();
            function Species(){
                captured=Object();descriptor.value=0;descriptor.writable=false;
                descriptor.enumerable=false;descriptor.configurable=false;
                Object.defineProperty(captured,"length",descriptor);return captured;
            }
            ctor[Symbol.species]=Species;source.constructor=ctor;
            try{source.splice(1,1,"x");return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+captured[0]+"|"+
                source.length+"|"+source.join(",")}
        })()
// case: species may alias the splice result to its source
(function(){
            var source=[1,2,3],ctor=Object();function Species(){return source}
            ctor[Symbol.species]=Species;source.constructor=ctor;
            var result=source.splice(1,1,9);
            return (result===source)+"|"+source.length+"|"+source[0]+"|"+source[1]+"|"+
                (2 in source);
        })()
// case: species construction mutation is observed by later slice reads
(function(){
            var source=[1,2,3],ctor=Object();
            function Species(){source[1]=8;return Object()}
            ctor[Symbol.species]=Species;source.constructor=ctor;
            var result=source.slice(0,3);return result[0]+","+result[1]+","+result[2];
        })()
// case: species may alias a slice result to its source
(function(){
            var source=[1,2,3],ctor=Object();function Species(){return source}
            ctor[Symbol.species]=Species;source.constructor=ctor;
            var result=source.slice(1,3);
            return (result===source)+"|"+source.length+"|"+source[0]+"|"+source[1]+"|"+
                (2 in source);
        })()
// case: a rejected result definition leaves the completed slice prefix externally observable
(function(){
            var captured,ctor=Object(),descriptor=Object();
            function Species(){
                captured=Object();descriptor.value="fixed";descriptor.writable=false;
                descriptor.enumerable=true;descriptor.configurable=false;
                Object.defineProperty(captured,"1",descriptor);return captured;
            }
            ctor[Symbol.species]=Species;var source=[1,2,3];source.constructor=ctor;
            try{source.slice();return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+captured[0]+"|"+
                captured[1]+"|"+("length" in captured)}
        })()
// case: constructor species lookup and constructor validation preserve abrupt order
(function(){
            function run(mode){
                var source=[1],ctor=Object();
                if(mode===0)source.__defineGetter__("constructor",function(){throw 81});
                if(mode===1){ctor.__defineGetter__(Symbol.species,function(){throw 82});source.constructor=ctor}
                if(mode===2){ctor[Symbol.species]=1;source.constructor=ctor}
                try{source.slice();return "missing"}
                catch(error){return typeof error+":"+(typeof error==="object"?error.name+":"+error.message:error)}
            }
            return run(0)+"|"+run(1)+"|"+run(2);
        })()
// MOVE_AND_PARTIAL_CASES
// case: splice shrink moves its tail forward before deleting and inserting
(function(){
            var source=Object(),log="",one,two;source.length=5;source[0]="a";
            source.__defineSetter__("1",function(value){log+="s1";one=value});
            source.__defineSetter__("2",function(value){log+="s2";two=value});
            source.__defineGetter__("3",function(){log+="g3";return "d"});
            source.__defineGetter__("4",function(){log+="g4";return "e"});
            var removed=Array.prototype.splice.call(source,1,2);
            return log+"|"+one+two+"|"+removed.length+"|"+source.length;
        })()
// case: splice growth moves its tail backward before ascending item Sets
(function(){
            var source=Object(),log="";source.length=4;source[0]="a";source[1]="b";
            source.__defineGetter__("2",function(){log+="g2";return "c"});
            source.__defineSetter__("2",function(value){log+="s2"+value});
            source.__defineGetter__("3",function(){log+="g3";return "d"});
            source.__defineSetter__("3",function(value){log+="s3"+value});
            source.__defineSetter__("4",function(value){log+="s4"+value});
            source.__defineSetter__("5",function(value){log+="s5"+value});
            Array.prototype.splice.call(source,1,1,"x","y","z");
            return log+"|"+source[1]+"|"+source.length;
        })()
// case: failed second shrink target Set preserves the completed first move
(function(){
            var source=Object(),descriptor=Object(),first;source.length=5;
            source[3]="d";source[4]="e";
            source.__defineSetter__("1",function(value){first=value});
            descriptor.value="fixed";descriptor.writable=false;descriptor.enumerable=true;
            descriptor.configurable=false;Object.defineProperty(source,"2",descriptor);
            try{Array.prototype.splice.call(source,1,2);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+first+"|"+source[2]+"|"+
                source[3]+"|"+source[4]+"|"+source.length}
        })()
// case: failed descending tail Delete preserves the move and earlier higher Delete
(function(){
            var source=["a","b","c","d","e"],descriptor=Object();
            descriptor.value="d";descriptor.writable=true;descriptor.enumerable=true;
            descriptor.configurable=false;Object.defineProperty(source,"3",descriptor);
            try{source.splice(1,3,"x");return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+source.length+"|"+
                source[1]+"|"+source[2]+"|"+source[3]+"|"+(4 in source)}
        })()
// case: failed later insertion preserves tail growth and earlier inserted item
(function(){
            var source=["a","b","c"],descriptor=Object();
            descriptor.value="c";descriptor.writable=false;descriptor.enumerable=true;
            descriptor.configurable=false;Object.defineProperty(source,"2",descriptor);
            try{source.splice(1,1,"x","y");return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+source.length+"|"+
                source[1]+"|"+source[2]+"|"+source[3]}
        })()
// case: failed final source length Set retains moves deletions and inserted items
(function(){
            var source=Object(),log="";source[0]="a";source[1]="b";source[2]="c";
            source.__defineGetter__("length",function(){return 3});
            source.__defineSetter__("length",function(value){log+="L"+value;throw 77});
            try{Array.prototype.splice.call(source,1,1,"x","y");return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log+"|"+source[1]+"|"+
                source[2]+"|"+source[3]}
        })()
// case: genuine Array Uint32 overflow keeps the ordinary high property
(function(){
            var source=Array(4294967295);
            try{source.splice(4294967295,0,"x");return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+source.length+"|"+
                source["4294967295"]+"|"+Object.prototype.hasOwnProperty.call(source,"4294967295")}
        })()
// case: failed later grow source Get preserves the completed high move and grown length
(function(){
            var source=["a","b","c","d"];
            source.__defineGetter__("2",function(){throw 92});
            try{source.splice(1,1,"x","y","z");return "missing"}
            catch(error){return typeof error+"|"+error+"|"+source.length+"|"+source[1]+"|"+
                source[5]}
        })()
// case: splice always Sets source length even when argc requests no mutation
(function(){
            var source=[1,2],descriptor=Object();descriptor.writable=false;
            Object.defineProperty(source,"length",descriptor);
            try{source.splice();return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+source.length+"|"+
                source[0]+source[1]}
        })()
// TO_SPLICED_ORDER_CASES
// case: toSpliced reads only the retained prefix and suffix in ascending order
(function(){
            var source=Object(),log="";source.length=5;
            source.__defineGetter__("0",function(){log+="g0";source[4]="changed";return "a"});
            source.__defineGetter__("1",function(){log+="bad1";throw 81});
            source.__defineGetter__("2",function(){log+="bad2";throw 82});
            source.__defineGetter__("3",function(){log+="g3";return "d"});source[4]="e";
            var result=Array.prototype.toSpliced.call(source,1,2,"x");
            return log+"|"+result.join(",")+"|"+source[4];
        })()
// case: toSpliced ignores constructor species and a replaced global Array
(function(){
            var intrinsic=Array,source=[1,2,3],log="";
            source.__defineGetter__("constructor",function(){log+="C";throw 91});
            globalThis.Array=function(){throw 92};
            var result=intrinsic.prototype.toSpliced.call(source,1,1,8);
            return intrinsic.isArray(result)+"|"+result.join(",")+"|"+log;
        })()
// case: toSpliced prefix mutation changes a later suffix query
(function(){
            var source=Object(),log="";source.length=4;
            source.__defineGetter__("0",function(){log+="0";source[3]="new";return "a"});
            source[1]="b";source[2]="c";source[3]="old";
            var result=Array.prototype.toSpliced.call(source,1,2,"x");
            return log+"|"+result[0]+result[1]+result[2];
        })()
// case: toSpliced signed-31-bit length failure precedes indexed reads
(function(){
            var source=Object(),log="";source.length=2147483648;
            source.__defineGetter__("0",function(){log+="G";return 1});
            try{Array.prototype.toSpliced.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: toSpliced may delete a MAX_SAFE range into an empty dense result without reads
(function(){
            var source=Object(),log="";source.length=9007199254740991;
            source.__defineGetter__("0",function(){log+="G";throw 93});
            var result=Array.prototype.toSpliced.call(source,0,9007199254740991);
            return result.length+"|"+log+"|"+Array.isArray(result);
        })()
// case: toSpliced dense result definitions bypass Array prototype numeric setters
(function(){
            var hits=0;Array.prototype.__defineSetter__("0",function(){hits++});
            var source=Array(1),result=source.toSpliced();
            var text=hits+"|"+Object.prototype.hasOwnProperty.call(result,"0")+"|"+
                (result[0]===undefined);
            delete Array.prototype[0];return text;
        })()
// GENERIC_LIMIT_AND_ERROR_CASES
// case: generic objects and primitive strings use the pinned boxing and UTF-16 paths
(function(){
            var source=Object();source.length=3;source[0]="a";source[2]="c";
            var a=Array.prototype.slice.call(source,0,3);
            var b=Array.prototype.toSpliced.call("A\uD83D\uDCA9Z",1,2,"x");
            return a.length+"|"+a[0]+"|"+(1 in a)+"|"+a[2]+"|"+
                b.length+"|"+b[0]+b[1]+b[2];
        })()
// case: splice on a String boxes and builds its result before immutable source mutation fails
(function(){
            try{Array.prototype.splice.call("ab",0,1);return "missing"}
            catch(error){return error.name+"|"+error.message}
        })()
// case: 64-bit property keys work without traversing the full MAX_SAFE range
(function(){
            var key="9007199254740990";
            var a=Object();a.length=9007199254740991;a[key]="S";
            var sliced=Array.prototype.slice.call(a,9007199254740990);
            var b=Object();b.length=9007199254740991;b[key]="P";
            var removed=Array.prototype.splice.call(b,9007199254740990,1);
            var c=Object();c.length=9007199254740991;c[key]="T";
            var copied=Array.prototype.toSpliced.call(c,0,9007199254740990);
            return sliced.length+":"+sliced[0]+"|"+removed.length+":"+removed[0]+":"+
                b.length+":"+(key in b)+"|"+copied.length+":"+copied[0];
        })()
// case: slice and splice allow a sparse 2^31 result before the first getter throws
(function(){
            function run(method){
                var source=Object();source.length=2147483648;
                source.__defineGetter__("0",function(){throw 61});
                try{method.call(source,0,2147483648);return "missing"}
                catch(error){return typeof error+":"+error}
            }
            return run(Array.prototype.slice)+"|"+run(Array.prototype.splice);
        })()
// case: default Array species rejects a 2^32 result before indexed reads
(function(){
            function run(method){
                var source=Object(),log="";source.length=4294967296;
                source.__defineGetter__("0",function(){log+="G";return 1});
                try{method.call(source,0,4294967296);return "missing"}
                catch(error){return error.name+":"+error.message+":"+log}
            }
            return run(Array.prototype.slice)+"|"+run(Array.prototype.splice);
        })()
// case: splice and toSpliced distinguish their MAX_SAFE TypeError messages
(function(){
            function run(method){
                var source=Object(),log="";source.length=9007199254740991;
                source.__defineGetter__("0",function(){log+="G";return 1});
                try{method.call(source,0,0,1);return "missing"}
                catch(error){return error.name+":"+error.message+":"+log}
            }
            return run(Array.prototype.splice)+"|"+run(Array.prototype.toSpliced);
        })()
// case: null receivers and Symbol or BigInt conversions preserve pinned native errors
(function(){
            function run(source){try{return source()}catch(error){return error.name+":"+error.message}}
            return run(function(){return Array.prototype.slice.call(null)})+"|"+
                run(function(){return [1].slice(Symbol("s"))})+"|"+
                run(function(){return [1].splice(0,1n)})+"|"+
                run(function(){return [1].toSpliced(Symbol("s"))});
        })()
// case: user conversion and indexed getter throws remain arbitrary values
(function(){
            function run(source){try{source();return "missing"}catch(error){return typeof error+":"+error}}
            var bound=Object();bound.valueOf=function(){throw 71};
            var source=Object();source.length=1;source.__defineGetter__("0",function(){throw 72});
            return run(function(){return [1].slice(bound)})+"|"+
                run(function(){return [1].splice(0,bound)})+"|"+
                run(function(){return Array.prototype.toSpliced.call(source)});
        })()
// case: recursive slice getter produces a catchable stack overflow
(function(){
            var source=Object();source.length=1;
            source.__defineGetter__("0",function(){
                return Array.prototype.slice.call(source,0,1)[0];
            });
            try{Array.prototype.slice.call(source,0,1);return "missing"}
            catch(error){return error.name+"|"+error.message}
        })()
