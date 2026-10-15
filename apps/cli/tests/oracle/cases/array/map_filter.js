// VALUE_CASES
// case: map transforms present elements while preserving holes and length
(function(){
            var source=[1,,3],log="";
            var result=source.map(function(value,index){log+=index;return value*2});
            return result.length+"|"+result[0]+"|"+(1 in result)+"|"+result[2]+"|"+
                (result!==source)+"|"+source[0]+"|"+source[2]+"|"+log;
        })()
// case: filter compacts selected present source values
(function(){
            var source=[1,,2,3,4],log="";
            var result=source.filter(function(value,index){log+=index;return value%2===1});
            return result.length+"|"+result[0]+"|"+result[1]+"|"+(2 in result)+"|"+
                (result!==source)+"|"+log;
        })()
// case: filter uses ToBoolean but stores the original value
(function(){
            var log="",truthy=Object();
            truthy.valueOf=function(){log+="v";throw 61};
            truthy.toString=function(){log+="s";throw 62};
            var result=[7].filter(function(){return truthy});
            return result.length+"|"+result[0]+"|"+log;
        })()
// case: map stores callback objects without coercion
(function(){
            var log="",marker=Object();
            marker.valueOf=function(){log+="v";throw 63};
            marker.toString=function(){log+="s";throw 64};
            var result=[1].map(function(){return marker});
            return (result[0]===marker)+"|"+log;
        })()
// case: map callback undefined creates a present result property
(function(){
            var result=[1].map(function(){});
            return result.length+"|"+(0 in result)+"|"+(result[0]===undefined);
        })()
// case: empty inputs allocate empty results without callbacks
(function(){
            var calls=0,callback=function(){calls++;return true};
            var mapped=[].map(callback),filtered=[].filter(callback);
            return mapped.length+"|"+filtered.length+"|"+calls+"|"+
                Array.isArray(mapped)+"|"+Array.isArray(filtered);
        })()
// ORDER_CASES
// case: holes are skipped while inherited values become own result properties
(function(){
            var proto=Object.create(Array.prototype),source=Array(4),log="";
            proto[1]=4;source[3]=8;Object.setPrototypeOf(source,proto);
            var mapped=source.map(function(value,index){log+="m"+index;return value+1});
            var filtered=source.filter(function(value,index){log+="f"+index;return true});
            return mapped.length+"|"+(0 in mapped)+"|"+mapped[1]+"|"+(2 in mapped)+"|"+
                mapped[3]+"|"+filtered.length+"|"+filtered[0]+"|"+filtered[1]+"|"+log;
        })()
// case: length is snapshotted and later HasProperty observes mutation
(function(){
            var source=[1,2,,4],log="";
            var result=source.map(function(value,index){
                log+=index+":"+value+",";
                if(index===0){delete source[1];source[2]=3;source[4]=5;source.length=5}
                return value*10;
            });
            return result.length+"|"+result[0]+"|"+(1 in result)+"|"+result[2]+"|"+
                result[3]+"|"+(4 in result)+"|"+log+"|"+source.length;
        })()
// case: filter copies the value captured before callback mutation
(function(){
            var source=[4];
            var result=source.filter(function(value,index,receiver){
                receiver[index]=9;return true;
            });
            return result[0]+"|"+source[0];
        })()
// case: callback value index boxed receiver and strict thisArg are exact
(function(){
            var marker=Object(),source="ab",log="";
            var mapped=Array.prototype.map.call(source,function(value,index,receiver){
                "use strict";
                log+="m"+(this===marker)+":"+value+":"+index+":"+
                    (typeof receiver)+":"+Object.prototype.toString.call(receiver)+",";
                return value;
            },marker);
            var filtered=Array.prototype.filter.call(source,function(value,index,receiver){
                "use strict";
                log+="f"+(this===undefined)+":"+(receiver!==source)+":"+index+",";
                return index===1;
            });
            return mapped[0]+mapped[1]+"|"+filtered[0]+"|"+log;
        })()
// case: callback validation precedes constructor and species access
(function(){
            var source=[1],log="";
            source.__defineGetter__("constructor",function(){log+="C";throw 71});
            try{source.map(1);return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: species allocation precedes indexed access and callbacks
(function(){
            var source=[1],ctor=Object(),descriptor=Object(),log="";
            function Species(length){log+="N"+length;return Object()}
            descriptor.get=function(){log+="S";return Species};
            Object.defineProperty(ctor,Symbol.species,descriptor);
            source.__defineGetter__("constructor",function(){log+="C";return ctor});
            source.__defineGetter__("0",function(){log+="G";return 5});
            var result=source.map(function(value){log+="F";return value+1});
            return result[0]+"|"+log;
        })()
// case: a callback throw stops before later indexed access
(function(){
            var source=[1,2],log="";
            source.__defineGetter__("1",function(){log+="G";throw 72});
            try{source.filter(function(){log+="F";throw 73});return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// SPECIES_CASES
// case: custom species receive map length or filter zero without a final length Set
(function(){
            function Species(length){var result=Object();result.arg=length;return result}
            var ctor=Object();ctor[Symbol.species]=Species;
            var left=[1,,3],right=[1,2,3];left.constructor=ctor;right.constructor=ctor;
            var mapped=left.map(function(value){return value*2});
            var filtered=right.filter(function(value){return value>1});
            return mapped.arg+"|"+mapped[0]+"|"+(1 in mapped)+"|"+mapped[2]+"|"+
                ("length" in mapped)+"|"+filtered.arg+"|"+filtered[0]+"|"+filtered[1]+"|"+
                ("length" in filtered);
        })()
// case: undefined constructor and null species fall back to base Arrays
(function(){
            var left=[1,2],right=[1,2],ctor=Object();
            left.constructor=undefined;ctor[Symbol.species]=null;right.constructor=ctor;
            var mapped=left.map(function(value){return value});
            var filtered=right.filter(function(){return true});
            return Array.isArray(mapped)+"|"+mapped.length+"|"+
                Array.isArray(filtered)+"|"+filtered.length;
        })()
// case: default species creation ignores a replaced global Array binding
(function(){
            var intrinsic=Array,map=Array.prototype.map,source=[1];
            source.constructor=undefined;
            globalThis.Array=function(){throw 79};
            var result=map.call(source,function(value){return value+1});
            return intrinsic.isArray(result)+"|"+result.length+"|"+result[0];
        })()
// case: generic receivers ignore their constructor property
(function(){
            var source=Object(),log="";source[0]=3;source.length=1;
            source.__defineGetter__("constructor",function(){log+="C";throw 81});
            var mapped=Array.prototype.map.call(source,function(value){return value+1});
            var filtered=Array.prototype.filter.call(source,function(){return true});
            return Array.isArray(mapped)+"|"+mapped[0]+"|"+
                Array.isArray(filtered)+"|"+filtered[0]+"|"+log;
        })()
// case: CreateDataProperty ignores an inherited result setter
(function(){
            var hits=0,proto=Object(),ctor=Object();
            proto.__defineSetter__("0",function(){hits++});
            function Species(){return Object.create(proto)}
            ctor[Symbol.species]=Species;
            var source=[2];source.constructor=ctor;
            var result=source.map(function(value){return value*3});
            return result[0]+"|"+hits+"|"+Object.prototype.hasOwnProperty.call(result,"0");
        })()
// case: a rejected result definition preserves callback order and stops traversal
(function(){
            var log="",ctor=Object(),captured;
            function Species(){
                var result=Object(),descriptor=Object();captured=result;
                descriptor.value=9;descriptor.writable=false;
                descriptor.enumerable=true;descriptor.configurable=false;
                Object.defineProperty(result,"1",descriptor);return result;
            }
            ctor[Symbol.species]=Species;
            var source=[1,2,3];source.constructor=ctor;
            source.__defineGetter__("2",function(){log+="G";throw 82});
            try{source.map(function(value,index){log+="F"+index;return value});return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log+"|"+
                captured[0]+"|"+captured[1]}
        })()
// GENERIC_CASES
// case: ordinary array-like results are defining-realm base Arrays
(function(){
            var source=Object();source[0]="a";source[2]="c";source.length=3;
            var mapped=Array.prototype.map.call(source,function(value,index,receiver){
                return (receiver===source)+":"+index+":"+value;
            });
            var filtered=Array.prototype.filter.call(source,function(value){return value==="c"});
            return Array.isArray(mapped)+"|"+mapped.length+"|"+mapped[0]+"|"+
                (1 in mapped)+"|"+mapped[2]+"|"+filtered.length+"|"+filtered[0];
        })()
// case: String receivers map and filter UTF-16 code units
(function(){
            var source="A\uD83D\uDCA9Z",boxed=true,indices="";
            var mapped=Array.prototype.map.call(source,function(value,index,receiver){
                boxed=boxed&&typeof receiver==="object";indices+=index;
                return value.charCodeAt(0);
            });
            var filtered=Array.prototype.filter.call(source,function(value){
                return value.charCodeAt(0)===0xDCA9;
            });
            return mapped.length+"|"+mapped[0]+"|"+mapped[1]+"|"+mapped[2]+"|"+
                mapped[3]+"|"+filtered[0].charCodeAt(0)+"|"+boxed+"|"+indices;
        })()
// case: zero-length primitive receivers allocate base Arrays without callbacks
(function(){
            var calls=0,callback=function(){calls++;return true};
            var mapped=Array.prototype.map.call(7,callback);
            var filtered=Array.prototype.filter.call(false,callback);
            return Array.isArray(mapped)+"|"+mapped.length+"|"+
                Array.isArray(filtered)+"|"+filtered.length+"|"+calls;
        })()
// case: MAX_SAFE_INTEGER map fails during base Array allocation
(function(){
            var source=Object(),log="";source.length=9007199254740991;
            source.__defineGetter__("0",function(){log+="G";return 1});
            try{Array.prototype.map.call(source,function(){log+="F";return 1});return "missing"}
            catch(error){return error.name+"|"+error.message+"|"+log}
        })()
// case: MAX_SAFE_INTEGER filter can abort at its first present index
(function(){
            var source=Object(),log="";source.length=9007199254740991;source[0]="x";
            try{Array.prototype.filter.call(source,function(value,index,receiver){
                log+=(receiver===source)+":"+index+":"+value;throw 91;
            });return "missing"}
            catch(error){return typeof error+"|"+error+"|"+log}
        })()
// ERROR_CASES
// case: null receiver
Array.prototype.map.call(null,function(){return 0})
// case: undefined receiver
Array.prototype.filter.call(undefined,function(){return true})
// case: missing map callback
[].map()
// case: undefined filter callback
[].filter(undefined)
// case: number callback
[].map(1)
// case: symbol callback
[].filter(Symbol('callback'))
// case: BigInt callback
[].map(0n)
// case: Symbol length wins before callback validation
(function(){var source=Object();source.length=Symbol('length');return Array.prototype.filter.call(source,1)})()
// case: primitive constructor is not a function
(function(){var source=[1];source.constructor=1;return source.map(function(value){return value})})()
// case: null constructor is not a function
(function(){var source=[1];source.constructor=null;return source.filter(function(){return true})})()
// case: primitive species is not a function
(function(){var source=[1],ctor=Object();ctor[Symbol.species]=Symbol('species');source.constructor=ctor;return source.map(function(value){return value})})()
// case: callable non-constructor species has the pinned constructor error
(function(){var source=[1],ctor=Object();ctor[Symbol.species]=Array.prototype.map;source.constructor=ctor;return source.filter(function(){return true})})()
// case: plain object species is not a constructor
(function(){var source=[1],ctor=Object();ctor[Symbol.species]=Object();source.constructor=ctor;return source.map(function(value){return value})})()
