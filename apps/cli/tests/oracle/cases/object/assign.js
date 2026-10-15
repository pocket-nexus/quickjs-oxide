// CONVERSION_CASES
// case: actual argc controls missing and single-argument calls while this is ignored
(function(){
            var missing;
            try{Object.assign()}catch(error){missing=error.name+":"+error.message}
            var target=Object(),receiver=Object();target.kept=1;
            return missing+":"+(Object.assign(target)===target)+":"+
                (Object.assign.call(receiver,target)===target)+":"+target.kept;
        })()
// case: target is boxed before sources and the exact target is returned
(function(){
            var target=Object(),source=Object(),last=Object();source.first=1;last.second=2;
            var returned=Object.assign(target,source,null,undefined,last);
            return (returned===target)+":"+target.first+":"+target.second+":"+
                Object.getOwnPropertyNames(target).join(",");
        })()
// case: primitive targets are boxed while preserving primitive payloads
(function(){
            function source(value){var object=Object();object.x=value;return object}
            var marker=Symbol("marker");
            var number=Object.assign(17,source(1));
            var boolean=Object.assign(false,source(2));
            var string=Object.assign("ab",source(3));
            var bigint=Object.assign(19n,source(4));
            var symbol=Object.assign(marker,source(5));
            return [
                typeof number,number.valueOf(),number.x,
                typeof boolean,boolean.valueOf(),boolean.x,
                typeof string,string.valueOf(),string[0]+string[1],string.x,
                typeof bigint,bigint.valueOf(),bigint.x,
                typeof symbol,symbol.valueOf()===marker,symbol.x
            ].join(":");
        })()
// case: nullish sources are skipped and primitive sources contribute only String indices
(function(){
            var symbol=Symbol("source"),target=Object();
            var returned=Object.assign(target,null,undefined,17,false,23n,symbol,"ab");
            return (returned===target)+":"+Object.getOwnPropertyNames(target).join(",")+":"+
                target[0]+target[1]+":"+Object.getOwnPropertySymbols(target).length;
        })()
// case: multiple sources are converted and copied strictly left to right
(function(){
            var log="",first=Object(),second=Object(),third=Object();
            first.__defineGetter__("x",function(){log+="first|";return 1});
            second.__defineGetter__("x",function(){log+="second|";return 2});
            third.__defineGetter__("y",function(){log+="third|";return 3});
            var target=Object.assign(Object(),first,second,third);
            return log+target.x+":"+target.y;
        })()
// PROPERTY_CASES
// case: own enumerable strings and Symbols copy in canonical order
(function(){
            var proto=Object(),source=Object.create(proto),hidden=Object();
            var first=Symbol("first"),second=Symbol("second");
            proto.inherited="I";source.z="Z";source[10]="ten";source[first]="S1";
            source[2]="two";source.a="A";
            hidden.value="H";hidden.writable=true;hidden.enumerable=false;hidden.configurable=true;
            Object.defineProperty(source,"hidden",hidden);source[second]="S2";
            var target=Object.assign(Object(),source),names=Object.getOwnPropertyNames(target);
            var symbols=Object.getOwnPropertySymbols(target);
            return names.join(",")+"|"+target[2]+":"+target[10]+":"+target.z+":"+target.a+"|"+
                (symbols[0]===first)+":"+(symbols[1]===second)+":"+target[first]+":"+target[second]+"|"+
                (target.hidden===undefined)+":"+(target.inherited===undefined);
        })()
// case: new target properties use assignment data flags and existing properties are updated
(function(){
            function bits(object,key){var d=Object.getOwnPropertyDescriptor(object,key);return d.writable+":"+d.enumerable+":"+d.configurable}
            var target=Object(),fixed=Object();target.existing=1;
            fixed.value=2;fixed.writable=true;fixed.enumerable=false;fixed.configurable=true;
            Object.defineProperty(target,"fixed",fixed);
            var source=Object();source.existing=3;source.created=4;source.fixed=5;
            Object.assign(target,source);
            return target.existing+":"+target.created+":"+target.fixed+"|"+
                bits(target,"existing")+"|"+bits(target,"created")+"|"+bits(target,"fixed");
        })()
// case: Set invokes inherited setters instead of defining an own property
(function(){
            var log="",proto=Object(),target=Object.create(proto),source=Object();
            proto.__defineSetter__("x",function(value){log+="set:"+value+":"+(this===target)});
            source.x=41;
            var returned=Object.assign(target,source);
            return (returned===target)+":"+log+":"+target.hasOwnProperty("x");
        })()
// case: the inherited __proto__ setter is used instead of CreateDataProperty
(function(){
            var target=Object(),nextPrototype=Object(),source=Object(),descriptor=Object();
            descriptor.value=nextPrototype;descriptor.writable=true;
            descriptor.enumerable=true;descriptor.configurable=true;
            Object.defineProperty(source,"__proto__",descriptor);
            var returned=Object.assign(target,source);
            return (returned===target)+":"+(Object.getPrototypeOf(target)===nextPrototype)+":"+
                target.hasOwnProperty("__proto__");
        })()
// case: assigning an object to itself preserves values and identities
(function(){
            var log="",stored=7,value=Object(),symbol=Symbol("self"),descriptor=Object();
            descriptor.enumerable=true;descriptor.configurable=true;
            descriptor.get=function(){log+="g";return stored};
            descriptor.set=function(next){log+="s";stored=next};
            Object.defineProperty(value,"x",descriptor);value[symbol]=value;
            var returned=Object.assign(value,value);
            return (returned===value)+":"+stored+":"+log+":"+(value[symbol]===value);
        })()
// MUTATION_CASES
// case: each ordinary source snapshots enumerable own keys before any Get
(function(){
            var getterThis=false,proto=Object(),source=Object.create(proto),target=Object();
            proto.b="inherited";
            source.__defineGetter__("a",function(){getterThis=this===source;delete source.b;source.late="late";return "A"});
            source.b="own";
            Object.assign(target,source);
            return Object.getOwnPropertyNames(target).join(",")+":"+target.a+":"+target.b+":"+
                (target.late===undefined)+":"+source.hasOwnProperty("b")+":"+getterThis;
        })()
// case: pinned ordinary fast path does not recheck later enumerability
(function(){
            var source=Object(),target=Object(),hidden=Object();
            source.__defineGetter__("a",function(){
                var b=Object();b.value="B";b.writable=true;b.enumerable=false;b.configurable=true;
                Object.defineProperty(source,"b",b);
                var h=Object();h.value="H";h.writable=true;h.enumerable=true;h.configurable=true;
                Object.defineProperty(source,"hidden",h);
                return "A";
            });
            source.b="before";
            hidden.value="hidden";hidden.writable=true;hidden.enumerable=false;hidden.configurable=true;
            Object.defineProperty(source,"hidden",hidden);
            Object.assign(target,source);
            return Object.getOwnPropertyNames(target).join(",")+":"+target.a+":"+target.b+":"+
                (target.hidden===undefined)+":"+Object.getOwnPropertyDescriptor(source,"b").enumerable+":"+
                Object.getOwnPropertyDescriptor(source,"hidden").enumerable;
        })()
// case: Get completes before Set for every string and Symbol key
(function(){
            var log="",source=Object(),target=Object(),symbol=Symbol("order");
            source.__defineGetter__("a",function(){log+="get-a|";return "A"});
            source.__defineGetter__("b",function(){log+="get-b|";return "B"});
            source.__defineGetter__(symbol,function(){log+="get-symbol|";return "S"});
            target.__defineSetter__("a",function(value){log+="set-a:"+value+"|"});
            target.__defineSetter__("b",function(value){log+="set-b:"+value+"|"});
            target.__defineSetter__(symbol,function(value){log+="set-symbol:"+value+"|"});
            Object.assign(target,source);
            return log+target.hasOwnProperty("a")+":"+target.hasOwnProperty("b");
        })()
// case: a target setter can affect the Get of a later snapshotted key
(function(){
            var log="",proto=Object(),source=Object.create(proto),target=Object();
            proto.b="prototype";source.a="A";source.b="own";
            target.__defineSetter__("a",function(value){log+="set-a:"+value+"|";delete source.b});
            target.__defineSetter__("b",function(value){log+="set-b:"+value+"|"});
            Object.assign(target,source);
            return log+source.hasOwnProperty("b");
        })()
// case: a later source is snapshotted only when its turn begins
(function(){
            var log="",first=Object(),second=Object(),target=Object();
            first.__defineGetter__("a",function(){log+="first|";second.added="later";return "A"});
            second.b="B";
            Object.assign(target,first,second);
            return log+Object.getOwnPropertyNames(target).join(",")+":"+target.added;
        })()
// EXOTIC_CASES
// case: Proxy ownKeys snapshot is followed by descriptor recheck then Get
(function(){
            var log="",visible=true,base={a:"A",b:"B"};
            var source=new Proxy(base,{
                ownKeys:function(){log+="ownKeys|";return ["a","b"]},
                getOwnPropertyDescriptor:function(target,key){log+="desc-"+key+":"+visible+"|";return {value:target[key],writable:true,enumerable:visible,configurable:true}},
                get:function(target,key){log+="get-"+key+"|";if(key==="a")visible=false;return target[key]}
            });
            var target=Object.assign({},source);
            return log+Object.getOwnPropertyNames(target).join(",")+":"+target.a+":"+(target.b===undefined);
        })()
// case: TypedArray integer indices are copied as enumerable own keys
(function(){
            var target=Object.assign({},new Uint8Array([7,8]));
            return Object.getOwnPropertyNames(target).join(",")+":"+target[0]+":"+target[1];
        })()
// PARTIAL_AND_ERROR_CASES
// case: nullish targets throw before any source getter executes
(function(){
            var calls=0,source=Object();source.__defineGetter__("x",function(){calls++;return 1});
            function row(target){try{Object.assign(target,source);return "missing"}catch(error){return error.name+":"+error.message+":"+calls}}
            return row(null)+"|"+row(undefined);
        })()
// case: source getter throw preserves prior writes and stops later keys
(function(){
            var target=Object(),source=Object();source.a=1;
            source.__defineGetter__("b",function(){throw 73});source.c=3;
            try{Object.assign(target,source)}catch(error){
                return (error===73)+":"+target.a+":"+(target.b===undefined)+":"+(target.c===undefined)+":"+
                    Object.getOwnPropertyNames(target).join(",");
            }
            return "missing";
        })()
// case: target setter throw happens after source Get and preserves earlier writes
(function(){
            var log="",target=Object(),source=Object();source.a=1;
            source.__defineGetter__("b",function(){log+="get-b|";return 2});source.c=3;
            target.__defineSetter__("b",function(value){log+="set-b:"+value+"|";throw new RangeError("setter")});
            try{Object.assign(target,source)}catch(error){
                return error.name+":"+error.message+"|"+log+"|"+target.a+":"+(target.c===undefined);
            }
            return "missing";
        })()
// case: read only and non extensible targets reject through Set
(function(){
            function readonly(){
                var target=Object(),descriptor=Object();descriptor.value=1;descriptor.writable=false;
                descriptor.enumerable=true;descriptor.configurable=true;Object.defineProperty(target,"x",descriptor);
                var source=Object();source.x=2;
                try{Object.assign(target,source)}catch(error){return error.name+":"+error.message+":"+target.x}
                return "missing";
            }
            function closed(){
                var target=Object();target.kept=1;Object.preventExtensions(target);
                var source=Object();source.kept=2;source.added=3;
                try{Object.assign(target,source)}catch(error){return error.name+":"+error.message+":"+target.kept+":"+(target.added===undefined)}
                return "missing";
            }
            return readonly()+"|"+closed();
        })()
// case: String indices and accessors without setters reject with Throw enabled
(function(){
            function stringIndex(){
                var source=Object();source[0]="changed";
                try{Object.assign("ab",source)}catch(error){return error.name+":"+error.message}
                return "missing";
            }
            function noSetter(){
                var target=Object(),descriptor=Object(),source=Object();
                descriptor.enumerable=true;descriptor.configurable=true;
                descriptor.get=function(){return 1};
                Object.defineProperty(target,"x",descriptor);source.x=2;
                try{Object.assign(target,source)}catch(error){return error.name+":"+error.message}
                return "missing";
            }
            return stringIndex()+"|"+noSetter();
        })()
// case: failure in a later source retains all earlier-source mutations
(function(){
            var target=Object(),first=Object(),second=Object();first.a=1;second.b=2;
            second.__defineGetter__("c",function(){throw "stop"});
            try{Object.assign(target,first,second)}catch(error){
                return error+":"+target.a+":"+target.b+":"+(target.c===undefined);
            }
            return "missing";
        })()
