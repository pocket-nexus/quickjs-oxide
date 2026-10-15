// prelude: string_case
// GRAPH_CASES
// case: prototype table order reaches the includes family
(function(){
            var selected=["length","at","charCodeAt","charAt","concat","codePointAt",
                "isWellFormed","toWellFormed","indexOf","lastIndexOf","includes",
                "endsWith","startsWith","toString","valueOf","constructor"];
            var keys=Object.getOwnPropertyNames(String.prototype),output=[],index=0;
            while(index<keys.length){
                if(selected.indexOf(keys[index])>=0)output.push(keys[index]);
                index++;
            }
            return output.join(",");
        })()
// case: method descriptors and function metadata are exact
(function(){
            var names=["includes","endsWith","startsWith"],output=[],index=0;
            while(index<names.length){
                var name=names[index],fn=String.prototype[name];
                output.push(name+":"+__bits(String.prototype,name)+":"+fn.name+":"+fn.length+
                    ":"+Object.getOwnPropertyNames(fn).join(",")+":"+
                    __bits(fn,"length")+":"+__bits(fn,"name")+":"+
                    (Object.getPrototypeOf(fn)===Function.prototype)+":"+
                    (typeof fn)+":"+__isConstructor(fn));
                index++;
            }
            return output.join("|");
        })()
// case: AutoInit materializes one stable callable per property
(function(){
            var first=String.prototype.includes,again=String.prototype.includes;
            var descriptor=Object.getOwnPropertyDescriptor(String.prototype,"includes");
            return [(first===again),(first===descriptor.value),
                (first===String.prototype.endsWith),(first===String.prototype.startsWith)].join("|");
        })()
// AUTOINIT_CASES
// case: lazy includes can be deleted before materialization
(function(){
            var deleted=delete String.prototype.includes;
            return [deleted,"includes" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"includes"),
                typeof String.prototype.includes].join("|");
        })()
// case: lazy endsWith assignment becomes an ordinary replacement
(function(){
            String.prototype.endsWith=17;
            return [String.prototype.endsWith,__bits(String.prototype,"endsWith"),
                Object.getOwnPropertyNames(String.prototype).indexOf("endsWith")].join("|");
        })()
// case: materialized startsWith remains deletable
(function(){
            var fn=String.prototype.startsWith,deleted=delete String.prototype.startsWith;
            return [typeof fn,deleted,"startsWith" in String.prototype,
                Object.prototype.hasOwnProperty.call(String.prototype,"startsWith")].join("|");
        })()
// VALUE_CASES
// case: includes values and clamped positions
(function(){return [
            "abc".includes(),"abc".includes(""),"abc".includes("",3),
            "abc".includes("",4),"abc".includes("bc"),"abc".includes("bc",1),
            "abc".includes("bc",2),"abc".includes("a",-1),"abc".includes("a",NaN),
            "abc".includes("a",-Infinity),"abc".includes("a",Infinity),
            "abc".includes("c",2.9),"abc".includes("a",4294967297)
        ].join("|")})()
// case: startsWith values and clamped positions
(function(){return [
            "abc".startsWith(),"abc".startsWith(""),"abc".startsWith("",3),
            "abc".startsWith("",4),"abc".startsWith("ab"),"abc".startsWith("bc",1),
            "abc".startsWith("bc",2),"abc".startsWith("a",-1),
            "abc".startsWith("a",NaN),"abc".startsWith("a",-Infinity),
            "abc".startsWith("c",Infinity),"abc".startsWith("c",2.9),
            "abc".startsWith("a",4294967297)
        ].join("|")})()
// case: endsWith values and end positions
(function(){return [
            "abc".endsWith(),"abc".endsWith(""),"abc".endsWith("",0),
            "abc".endsWith("",4),"abc".endsWith("bc"),"abc".endsWith("ab",2),
            "abc".endsWith("bc",2),"abc".endsWith("a",1),"abc".endsWith("a",-1),
            "abc".endsWith("",NaN),"abc".endsWith("abc",Infinity),
            "abc".endsWith("a",1.9),"abc".endsWith("abc",4294967297)
        ].join("|")})()
// case: UTF-16 searches operate on raw code-unit boundaries
(function(){
            var source="A\ud83d\ude00\ud800Z";
            return [source.includes("\ud83d"),source.includes("\ude00"),
                source.includes("\ude00\ud800"),source.includes("\ud800Z"),
                source.startsWith("\ud83d",1),source.startsWith("\ude00",2),
                source.startsWith("\ud800",3),source.endsWith("\ude00",3),
                source.endsWith("\ud800",4),source.endsWith("\ud800Z")].join("|");
        })()
// case: large rope searches cross leaf and rebalance boundaries
(function(){
            var source="",index=0;
            while(index<9000){source+="a";index++}
            source+="b\ud800Z";
            return [source.length,source.includes("ab",8998),source.includes("b\ud800",9000),
                source.startsWith("ab",8999),source.startsWith("b\ud800",9000),
                source.endsWith("b\ud800Z"),source.endsWith("ab",9001),
                source.includes("\ud800Z",9001)].join("|");
        })()
// case: borrowed methods remain generic and ignore this method owner
(function(){return [
            String.prototype.includes.call(123,"2"),
            String.prototype.startsWith.call(true,"tr"),
            String.prototype.endsWith.call(1n,"1")
        ].join("|")})()
// ORDER_CASES
// case: receiver then match then search then position is the exact order
(function(){
            var log="",receiver=Object(),search=Object(),position=Object(),descriptor=Object();
            receiver[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "abc"};
            descriptor.configurable=true;
            descriptor.get=function(){log+="get-match;";return false};
            Object.defineProperty(search,Symbol.match,descriptor);
            search[Symbol.toPrimitive]=function(hint){log+="search:"+hint+";";return "b"};
            position[Symbol.toPrimitive]=function(hint){log+="position:"+hint+";";return 1};
            var result=String.prototype.includes.call(receiver,search,position);
            return result+"|"+log;
        })()
// case: all three selectors share the same observable conversion prefix
(function(){
            var log="";
            function run(name){
                var receiver=Object(),search=Object(),position=Object(),descriptor=Object();
                receiver[Symbol.toPrimitive]=function(hint){log+=name+":r:"+hint+";";return "abc"};
                descriptor.get=function(){log+=name+":m;";return null};
                descriptor.configurable=true;Object.defineProperty(search,Symbol.match,descriptor);
                search[Symbol.toPrimitive]=function(hint){log+=name+":s:"+hint+";";return "b"};
                position[Symbol.toPrimitive]=function(hint){log+=name+":p:"+hint+";";return 1};
                return String.prototype[name].call(receiver,search,position);
            }
            return [run("includes"),run("startsWith"),run("endsWith"),log].join("|");
        })()
// case: non-object search skips Symbol.match lookup entirely
(function(){
            var hits=0,descriptor=Object();
            descriptor.configurable=true;descriptor.get=function(){hits++;throw "wrong"};
            Object.defineProperty(String.prototype,Symbol.match,descriptor);
            var result="abc".includes("b");
            delete String.prototype[Symbol.match];
            return result+"|"+hits;
        })()
// case: undefined match falls through while false and null override IsRegExp
(function(){
            function run(match){
                var log="",search=Object();search[Symbol.match]=match;
                search[Symbol.toPrimitive]=function(hint){log+=hint+";";return "b"};
                return "abc".includes(search)+":"+log;
            }
            return [run(undefined),run(false),run(null)].join("|");
        })()
// case: truthy match short-circuits search and position conversion
(function(){
            var log="",search=Object(),position=Object();
            search[Symbol.match]=true;
            search[Symbol.toPrimitive]=function(){log+="search;";throw "wrong"};
            position[Symbol.toPrimitive]=function(){log+="position;";throw "wrong"};
            try{"abc".includes(search,position)}catch(error){return error.name+":"+error.message+"|"+log}
            return "missing";
        })()
// case: truthy object match uses ToBoolean without coercing that object
(function(){
            var log="",search=Object(),match=Object();
            match[Symbol.toPrimitive]=function(){log+="match-convert;";return false};
            search[Symbol.match]=match;
            search[Symbol.toPrimitive]=function(){log+="search-convert;";return "b"};
            try{"abc".startsWith(search)}catch(error){return error.name+":"+error.message+"|"+log}
            return "missing";
        })()
// case: explicit undefined position is not converted
(function(){return [
            "abc".includes("a",undefined),"abc".startsWith("a",undefined),
            "abc".endsWith("c",undefined)
        ].join("|")})()
// case: receiver failure precedes every search hook
(function(){
            var log="",receiver=Object(),search=Object(),descriptor=Object();
            receiver[Symbol.toPrimitive]=function(){log+="receiver;";throw "receiver-throw"};
            descriptor.get=function(){log+="match;";return false};descriptor.configurable=true;
            Object.defineProperty(search,Symbol.match,descriptor);
            try{String.prototype.endsWith.call(receiver,search)}catch(error){return error+"|"+log}
            return "missing";
        })()
// case: search conversion failure prevents position conversion
(function(){
            var log="",search=Object(),position=Object();search[Symbol.match]=false;
            search[Symbol.toPrimitive]=function(){log+="search;";throw "search-throw"};
            position[Symbol.toPrimitive]=function(){log+="position;";return 0};
            try{"abc".includes(search,position)}catch(error){return error+"|"+log}
            return "missing";
        })()
// REGEXP_CASES
// case: real RegExp is rejected before position conversion
(function(){
            var log="",position=Object();position[Symbol.toPrimitive]=function(){log+="position;";return 0};
            try{"abc".includes(RegExp("b"),position)}catch(error){return error.name+":"+error.message+"|"+log}
            return "missing";
        })()
// case: RegExp Symbol.match false and null override the internal brand
(function(){
            var first=RegExp("b");first[Symbol.match]=false;
            var second=RegExp("b");second[Symbol.match]=null;
            return ["abc/b/".includes(first),"abc/b/".startsWith(second,3)].join("|");
        })()
// case: RegExp undefined match falls back to its internal brand
(function(){
            var value=RegExp("b");value[Symbol.match]=undefined;
            try{"abc".endsWith(value)}catch(error){return error.name+":"+error.message}
            return "missing";
        })()
// ERROR_CASES
// case: includes rejects null receiver
String.prototype.includes.call(null,'x')
// case: startsWith rejects undefined receiver
String.prototype.startsWith.call(undefined,'x')
// case: endsWith rejects Symbol search
'abc'.endsWith(Symbol('x'))
// case: truthy Symbol.match rejects search
(function(){var search=Object();search[Symbol.match]=1;return "abc".includes(search)})()
// case: Symbol.match getter throw is preserved
(function(){
            var search=Object(),descriptor=Object();descriptor.get=function(){throw 91};
            descriptor.configurable=true;Object.defineProperty(search,Symbol.match,descriptor);
            return "abc".startsWith(search);
        })()
// case: search ToPrimitive object result is rejected
(function(){
            var search=Object();search[Symbol.match]=false;
            search[Symbol.toPrimitive]=function(){return Object()};
            return "abc".endsWith(search);
        })()
// case: includes rejects BigInt position
'abc'.includes('a',1n)
// case: startsWith rejects Symbol position
'abc'.startsWith('a',Symbol('p'))
// case: position conversion throw is preserved
(function(){
            var position=Object();position[Symbol.toPrimitive]=function(){throw "position"};
            return "abc".endsWith("c",position);
        })()
// case: includes is not a constructor
new String.prototype.includes()
// case: endsWith is not a constructor
new String.prototype.endsWith()
// case: startsWith is not a constructor
new String.prototype.startsWith()
// EXOTIC_CASES
// case: Proxy exposes match Get then search ToPrimitive order
(function(){
            var log="",target=Object();target[Symbol.match]=false;
            target[Symbol.toPrimitive]=function(hint){log+="convert:"+hint+";";return "b"};
            var search=new Proxy(target,{get:function(object,key,receiver){
                log+="get:"+String(key)+";";return Reflect.get(object,key,receiver)}});
            return "abc".includes(search)+"|"+log;
        })()
// case: Proxy receiver conversion completes before Proxy search match trap
(function(){
            var log="",receiverTarget=Object(),searchTarget=Object();
            receiverTarget[Symbol.toPrimitive]=function(hint){log+="receiver:"+hint+";";return "abc"};
            searchTarget[Symbol.match]=false;
            searchTarget[Symbol.toPrimitive]=function(hint){log+="search:"+hint+";";return "b"};
            var receiver=new Proxy(receiverTarget,{get:function(object,key,recv){
                log+="receiver-get:"+String(key)+";";return Reflect.get(object,key,recv)}});
            var search=new Proxy(searchTarget,{get:function(object,key,recv){
                log+="search-get:"+String(key)+";";return Reflect.get(object,key,recv)}});
            return String.prototype.startsWith.call(receiver,search,1)+"|"+log;
        })()
// case: TypedArrays are generic receiver search and position values
(function(){
            return [
                String.prototype.includes.call(new Uint8Array([1,2]),","),
                "x65y".includes(new Uint8Array([65])),
                "abc".startsWith("b",new Uint8Array([1])),
                "abc".endsWith("b",new Uint8Array([2]))
            ].join("|");
        })()
// case: TypedArray prototype match lookup precedes string conversion
(function(){
            var log="",search=Uint8Array.prototype,descriptor=Object();
            descriptor.configurable=true;
            descriptor.get=function(){log+="match;";return false};
            Object.defineProperty(search,Symbol.match,descriptor);
            search[Symbol.toPrimitive]=function(hint){log+="convert:"+hint+";";return "b"};
            return "abc".includes(search)+"|"+log;
        })()
// case: Proxy Get invariant applies to a fixed Symbol.match property
(function(){
            var target=Object(),descriptor=Object();
            descriptor.value=false;descriptor.writable=false;descriptor.configurable=false;
            Object.defineProperty(target,Symbol.match,descriptor);
            var search=new Proxy(target,{get:function(){return true}});
            try{"abc".includes(search)}catch(error){return error.name+":"+error.message}
            return "missing";
        })()
// case: revoked Proxy rejection occurs during Symbol.match lookup
(function(){
            var pair=Proxy.revocable(Object(),Object());pair.revoke();
            try{"abc".includes(pair.proxy)}catch(error){return error.name+":"+error.message}
            return "missing";
        })()
// case: Proxy wrapping RegExp is not branded when Symbol.match is undefined
(function(){
            var target=RegExp("b");target.toString=function(){return "b"};
            var search=new Proxy(target,{get:function(object,key,receiver){
                if(key===Symbol.match)return undefined;
                return Reflect.get(object,key,receiver);
            }});
            return "abc".includes(search);
        })()
// STACK_CASES
// case: recursive Symbol.match getter throws catchably and runtime recovers
(function(){
            var search=Object(),descriptor=Object(),errorName="",errorMessage="";
            descriptor.configurable=true;
            descriptor.get=function(){return "x".includes(search)};
            Object.defineProperty(search,Symbol.match,descriptor);
            try{"x".includes(search)}catch(error){errorName=error.name;errorMessage=error.message}
            return [errorName,errorMessage,"abc".includes("b"),
                "abc".startsWith("a"),"abc".endsWith("c")].join("|");
        })()
// case: interleaved includes family recursion cannot bypass recovery guard
(function(){
            var search=Object(),descriptor=Object(),depth=0,errorName="";
            descriptor.configurable=true;
            descriptor.get=function(){
                depth++;
                if(depth%3===0)return "x".includes(search);
                if(depth%3===1)return "x".startsWith(search);
                return "x".endsWith(search);
            };
            Object.defineProperty(search,Symbol.match,descriptor);
            try{"x".includes(search)}catch(error){errorName=error.name}
            return [errorName,"recovered".includes("cover"),"recovered".endsWith("red")].join("|");
        })()
