// VALIDATION_CASES
// case: sort validates comparefn before ToObject and length
(function(){
            var source=Object(),log="";
            source.__defineGetter__("length",function(){log+="L";return 0});
            var first,second;
            try{Array.prototype.sort.call(source,1);first="missing"}
            catch(error){first=error.name+":"+error.message}
            try{Array.prototype.sort.call(null,1);second="missing"}
            catch(error){second=error.name+":"+error.message}
            return first+"|"+second+"|"+log;
        })()
// case: toSorted validates comparefn before ToObject and length
(function(){
            var source=Object(),log="";
            source.__defineGetter__("length",function(){log+="L";return 0});
            var first,second;
            try{Array.prototype.toSorted.call(source,false);first="missing"}
            catch(error){first=error.name+":"+error.message}
            try{Array.prototype.toSorted.call(undefined,false);second="missing"}
            catch(error){second=error.name+":"+error.message}
            return first+"|"+second+"|"+log;
        })()
// case: omitted and explicit undefined select the default comparator
(function(){
            var first=[2,10,1],second=[2,10,1];
            first.sort();second.sort(undefined);
            return first[0]+","+first[1]+","+first[2]+"|"+
                second[0]+","+second[1]+","+second[2];
        })()
// DEFAULT_CASES
// case: default sort compares numeric-looking values lexicographically
(function(){
            var source=[20,3,100,11],same=source.sort()===source;
            var copy=source.toSorted();
            return same+"|"+source[0]+","+source[1]+","+source[2]+","+source[3]+"|"+
                copy[0]+","+copy[1]+","+copy[2]+","+copy[3];
        })()
// case: default comparison uses UTF16 code units including lone surrogates
(function(){
            var source=["\uE000","\uD800","a","\uDC00","\uD7FF"];
            var result=source.toSorted(),codes="",i;
            for(i=0;i<result.length;i++)codes+=(i?",":"")+result[i].charCodeAt(0);
            return codes+"|"+source[0].charCodeAt(0)+"|"+result.length;
        })()
// case: sort places defined values then undefined then holes
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=Array(6);source[0]=undefined;source[2]="b";source[4]="a";
            source.sort();
            return source.length+"|"+source[0]+"|"+source[1]+"|"+
                (source[2]===undefined)+"|"+own(source,2)+"|"+
                own(source,3)+own(source,4)+own(source,5);
        })()
// case: toSorted densifies both undefined values and source holes
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=Array(6);source[0]=undefined;source[2]="b";source[4]="a";
            var result=source.toSorted(),bits="",i;
            for(i=0;i<result.length;i++)bits+=Number(own(result,i))+Number(result[i]===undefined);
            return result[0]+"|"+result[1]+"|"+bits+"|"+
                own(source,1)+own(source,3)+own(source,5);
        })()
// case: default ToString is cached per slot but not skipped for the same object
(function(){
            var value=Object(),log="";
            value.toString=function(){log+="T";return "x"};
            var source=[value,value];source.sort();
            return log+"|"+(source[0]===value)+"|"+(source[1]===value);
        })()
// CUSTOM_CASES
// case: stable custom ties preserve original object order
(function(){
            var a=Object(),b=Object(),c=Object(),d=Object();
            a.id="a";a.key=1;b.id="b";b.key=0;c.id="c";c.key=1;d.id="d";d.key=0;
            var source=[a,b,c,d],result=source.toSorted(function(x,y){return x.key-y.key});
            return result[0].id+result[1].id+result[2].id+result[3].id+"|"+
                source[0].id+source[1].id+source[2].id+source[3].id;
        })()
// case: pinned rqsort comparator order for three values
(function(){
            var source=[3,2,1],log="";
            source.sort(function(a,b){log+=a+":"+b+",";return a-b});
            return log+"|"+source[0]+source[1]+source[2];
        })()
// case: pinned rqsort comparator order for seven values
(function(){
            var source=[7,6,5,4,3,2,1],log="";
            source.sort(function(a,b){log+=a+":"+b+",";return a-b});
            return log+"|"+source[0]+source[1]+source[2]+source[3]+source[4]+source[5]+source[6];
        })()
// case: raw-identical custom values bypass comparator calls
(function(){
            var object=Object(),string="same",source=[object,object,1,1,string,string],log="";
            source.sort(function(a,b){
                function tag(value){return value===object?"object":typeof value+":"+(""+value)}
                log+=tag(a)+">"+tag(b)+",";return 0;
            });
            return log+"|"+(source[0]===object)+"|"+source[2]+"|"+source[4];
        })()
// case: atomized literals share raw identity while tagged integers and dynamic strings do not
(function(){
            function calls(a,b){var count=0;[a,b].sort(function(){count++;return 0});return count}
            function left(){return "same"}function right(){return "same"}
            function numeric(){return "1"}
            var a="sa",b="me";
            return calls("same","same")+"|"+calls(left(),right())+"|"+
                calls("1","1")+"|"+calls(numeric(),numeric())+"|"+
                calls("2147483648","2147483648")+"|"+calls(`same`,`same`)+"|"+
                calls(`1`,`1`)+"|"+calls(a+b,a+b);
        })()
// case: custom result objects are converted with numeric hint
(function(){
            var result=Object(),log="",source=[2,1];
            result.valueOf=function(){log+="N";return -1};
            result.toString=function(){log+="T";return "1"};
            source.sort(function(a,b){log+="C"+a+b;return result});
            return log+"|"+source[0]+","+source[1];
        })()
// case: NaN comparator results are stable ties
(function(){
            var source=[3,1,2],log="";
            source.sort(function(a,b){log+=a+":"+b+",";return 0/0});
            return source[0]+","+source[1]+","+source[2]+"|"+log;
        })()
// ORDER_AND_PARTIAL_CASES
// case: collect compare defined Sets undefined Sets and Delete run in pinned order
(function(){
            function own(object,key){return Object.prototype.hasOwnProperty.call(object,key)}
            var source=Object(),proto=Object(),descriptor=Object(),log="";
            Object.setPrototypeOf(source,proto);source.length=5;
            descriptor.get=function(){log+="G0";return "b"};
            descriptor.set=function(value){log+="S0"+value};descriptor.configurable=true;
            Object.defineProperty(source,"0",descriptor);
            descriptor=Object();descriptor.get=function(){log+="G1";return undefined};
            descriptor.set=function(value){log+="S1"+value};descriptor.configurable=true;
            Object.defineProperty(source,"1",descriptor);
            proto.__defineSetter__("2",function(value){log+="U2"+(value===undefined)});
            descriptor=Object();descriptor.get=function(){log+="G3";return "a"};
            descriptor.set=function(value){log+="S3"+(value===undefined)};descriptor.configurable=true;
            Object.defineProperty(source,"3",descriptor);
            Array.prototype.sort.call(source,function(a,b){log+="C";source[4]="made";return a<b?-1:1});
            return log+"|"+own(source,2)+"|"+own(source,4)+"|"+source.length;
        })()
// case: unchanged original position skips Set while undefined does not
(function(){
            var first=Object(),second=Object(),descriptor=Object(),log="";
            first.length=1;descriptor.get=function(){log+="G";return "a"};
            descriptor.set=function(value){log+="S"};descriptor.configurable=true;
            Object.defineProperty(first,"0",descriptor);Array.prototype.sort.call(first);
            descriptor=Object();second.length=1;
            descriptor.get=function(){log+="U";return undefined};
            descriptor.set=function(value){log+="W"+(value===undefined)};descriptor.configurable=true;
            Object.defineProperty(second,"0",descriptor);Array.prototype.sort.call(second);
            return log;
        })()
// case: failed defined Set preserves an earlier moved value
(function(){
            var source=Object(),descriptor=Object();source.length=3;source[0]="c";source[2]="b";
            descriptor.value="a";descriptor.writable=false;
            descriptor.enumerable=true;descriptor.configurable=false;
            Object.defineProperty(source,"1",descriptor);
            try{Array.prototype.sort.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+source[0]+source[1]+source[2]}
        })()
// case: failed undefined Set preserves completed defined writes
(function(){
            var source=Object(),descriptor=Object();source.length=3;source[0]="b";source[1]="a";
            descriptor.value=undefined;descriptor.writable=false;
            descriptor.enumerable=true;descriptor.configurable=false;
            Object.defineProperty(source,"2",descriptor);
            try{Array.prototype.sort.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+source[0]+source[1]+"|"+(source[2]===undefined)}
        })()
// case: failed Delete preserves defined and undefined writes
(function(){
            var source=Object(),descriptor=Object(),installed=false;
            source.length=4;source[0]="b";source[1]="a";source[2]=undefined;
            try{
                Array.prototype.sort.call(source,function(a,b){
                    if(!installed){installed=true;descriptor.value="fixed";descriptor.writable=true;
                        descriptor.enumerable=true;descriptor.configurable=false;
                        Object.defineProperty(source,"3",descriptor)}
                    return a<b?-1:1;
                });return "missing";
            }catch(error){
                return error.name+"|"+error.message+"|"+source[0]+source[1]+"|"+
                    (source[2]===undefined)+"|"+source[3];
            }
        })()
// case: getters and comparator mutate within a snapshotted length
(function(){
            var source=Object(),descriptor=Object(),log="",written;
            source.length=3;source[2]="a";
            descriptor.get=function(){log+="G";source[1]="b";source[4]="outside";source.length=5;return "c"};
            descriptor.set=function(value){log+="S";written=value};descriptor.configurable=true;
            Object.defineProperty(source,"0",descriptor);
            Array.prototype.sort.call(source,function(a,b){log+="C";source[2]="changed";return a<b?-1:1});
            return log+"|"+source.length+"|"+written+"|"+source[1]+"|"+source[2]+"|"+source[4];
        })()
// GENERIC_AND_LIMIT_CASES
// case: toSorted ignores constructor and species
(function(){
            var source=[2,1],ctor=Object(),log="";
            ctor.__defineGetter__(Symbol.species,function(){log+="S";throw 71});
            source.constructor=ctor;
            var result=source.toSorted();
            var other=[4,3];other.__defineGetter__("constructor",function(){log+="C";throw 72});
            var second=other.toSorted();
            return result[0]+result[1]+"|"+second[0]+second[1]+"|"+log+"|"+Array.isArray(result);
        })()
// case: primitive receivers are boxed or copied
(function(){
            var boxed=Array.prototype.sort.call(7),copy=Array.prototype.toSorted.call(false);
            return Object.prototype.toString.call(boxed)+"|"+boxed.valueOf()+"|"+
                Array.isArray(copy)+"|"+copy.length;
        })()
// case: String toSorted copies code units while sort rejects immutable writes
(function(){
            var errorText;
            try{Array.prototype.sort.call("ba");errorText="missing"}
            catch(error){errorText=error.name+":"+error.message}
            var result=Array.prototype.toSorted.call("cba");
            return errorText+"|"+result[0]+result[1]+result[2]+"|"+result.length;
        })()
// case: toSorted INT32 limit fails before indexed reads
(function(){
            var source=Object(),log="";source.length=2147483648;
            source.__defineGetter__("0",function(){log+="G";throw 73});
            try{Array.prototype.toSorted.call(source);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: sort accepts MAX_SAFE length and begins at full u64 index zero
(function(){
            var source=Object(),log="";source.length=9007199254740991;
            source.__defineGetter__("0",function(){log+="G";throw 74});
            try{Array.prototype.sort.call(source);return "missing"}
            catch(error){return typeof error+"|"+(""+error)+"|"+log}
        })()
// ERROR_CASES
// case: Symbol comparator result
[2,1].sort(function(){return Symbol('order')})
// case: BigInt comparator result
[2,1].sort(function(){return 1n})
// case: default Symbol element ToString
[Symbol('value'),'a'].sort()
// case: arbitrary comparator throw
[2,1].sort(function(){throw 'compare boom'})
// case: source getter user throw before comparison
(function(){var source=Object();source.length=2;source.__defineGetter__('0',function(){throw 81});return Array.prototype.sort.call(source,function(){throw 82})})()
// RECURSION_CASES
// case: recursive comparator produces catchable stack overflow
(function(){
        function compare(){[2,1].sort(compare);return 0}
        try{[2,1].sort(compare);return "missing"}
        catch(error){return error.name+"|"+error.message}
    })()
