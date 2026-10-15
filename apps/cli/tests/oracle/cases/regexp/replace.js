// prelude: regexp_replace
// METADATA_CASES
// case: RegExp Symbol.replace exposes pinned descriptor metadata and Symbol order
(function(){
            var fn=RegExp.prototype[Symbol.replace],
                keys=Reflect.ownKeys(RegExp.prototype),
                selected=[],index=0,key;
            while(index<keys.length){
                key=keys[index++];
                if(key==="exec"||key==="compile"||key==="test"||key==="toString"||
                    key==="constructor"||key===Symbol.replace||key===Symbol.match||
                    key===Symbol.search||key===Symbol.split)
                    selected[selected.length]=String(key);
            }
            return [
                selected.join(","),
                __bits(RegExp.prototype,Symbol.replace),fn.name,fn.length,
                Object.getOwnPropertyNames(fn).join(","),
                __bits(fn,"name"),__bits(fn,"length"),
                __isConstructor(fn),
                Object.prototype.hasOwnProperty.call(fn,"prototype")
            ].join("|");
        })()
// case: RegExp Symbol.replace AutoInit identity is stable and independently replaceable
(function(){
            var first=RegExp.prototype[Symbol.replace],
                second=RegExp.prototype[Symbol.replace],
                stable=first===second,
                deleted=delete RegExp.prototype[Symbol.replace];
            RegExp.prototype[Symbol.replace]=17;
            return [stable,deleted,RegExp.prototype[Symbol.replace],
                __bits(RegExp.prototype,Symbol.replace),
                typeof first,first.name,first.length].join("|");
        })()
// PREFIX_AND_ABSTRACT_EXEC_CASES
// case: RegExp Symbol.replace validates receiver then converts input replacement and flags
(function(){
            var primitiveLog="",primitiveInput=Object(),primitiveReplacement=Object();
            primitiveInput.toString=function(){primitiveLog+="BAD-input;";return "x"};
            primitiveReplacement.toString=function(){primitiveLog+="BAD-replacement;";return "y"};
            var primitive=__completion(function(){
                return RegExp.prototype[Symbol.replace].call(
                    1,primitiveInput,primitiveReplacement);
            });

            var log="",receiver=Object(),input=Object(),replacement=Object(),flags=Object();
            input.toString=function(){log+="input-string;";return "abc"};
            replacement.toString=function(){log+="replacement-string;";return "X"};
            Object.defineProperty(receiver,"flags",{get:function(){
                log+="flags-get;";return flags;
            }});
            flags.toString=function(){log+="flags-string;";return ""};
            Object.defineProperty(receiver,"exec",{get:function(){
                log+="exec-get;";
                return function(value){
                    log+="exec-call:"+(this===receiver)+":"+value+":"+arguments.length+";";
                    return null;
                };
            }});
            var result=RegExp.prototype[Symbol.replace].call(receiver,input,replacement);
            return [primitive,primitiveLog,result,log].join("|");
        })()
// case: abstract RegExpExec accepts null and rejects primitive results after the exact Get
(function(){
            function run(result){
                var log="",receiver=Object();receiver.flags="";
                Object.defineProperty(receiver,"exec",{get:function(){
                    log+="get;";
                    return function(value){
                        log+="call:"+(this===receiver)+":"+value+";";
                        return result;
                    };
                }});
                return __completion(function(){
                    return RegExp.prototype[Symbol.replace].call(receiver,"abc","X");
                })+":"+log;
            }
            var ordinary=Object();ordinary.flags="";ordinary.exec=null;
            return [
                run(null),run(1),run("match"),
                __completion(function(){
                    return RegExp.prototype[Symbol.replace].call(ordinary,"abc","X");
                })
            ].join("|");
        })()
// case: callable replacement is not converted before flags or execution
(function(){
            var log="",receiver=Object(),calls=0;
            receiver.flags="";
            receiver.exec=function(){
                calls++;
                return {length:1,0:"b",index:1,groups:undefined};
            };
            function replacement(match,position,input){
                "use strict";
                log+="call:"+(this===undefined)+":"+match+":"+position+":"+input+":"+
                    arguments.length+";";
                var result=Object();
                result.toString=function(){log+="result-string;";return "X"};
                return result;
            }
            replacement.toString=function(){log+="BAD-function-string;";throw "function"};
            var result=RegExp.prototype[Symbol.replace].call(receiver,"abc",replacement);
            return result+"|"+calls+"|"+log;
        })()
// case: Function.prototype.call trampoline preserves both logical native stack frames
(function(){
            function countNativeCalls(stack){
                var text=String(stack),needle="at call (native)",count=0,index=0;
                while((index=text.indexOf(needle,index))>=0){
                    count++;index+=needle.length;
                }
                return count;
            }
            function capture(callback){
                try{callback();return "return"}
                catch(error){
                    return [
                        error.name,error.message,countNativeCalls(error.stack),
                        String(error.stack).indexOf("at boom")>=0
                    ].join(":");
                }
            }
            var thrown=capture(function(){
                Function.prototype.call.call(function boom(){throw Error("x")},null);
            });
            var noncallable=capture(function(){
                Function.prototype.call.call(1,null);
            });
            var c=Function.prototype.call,f=function(){return String(42)};
            var deep=c.call(
                c,c,c,c,c,c,c,c,c,c,c,c,c,c,c,c,c,c,c,c,f,null
            );
            return thrown+"|"+noncallable+"|"+deep;
        })()
// TWO_PHASE_CASES
// case: global replacement collects all exec results before processing and rereads result zero
(function(){
            var log="",receiver=Object(),state=7,calls=0,group1=Object(),group2=Object();
            receiver.flags="g";
            Object.defineProperty(receiver,"lastIndex",{
                get:function(){log+="last-get:"+__show(state)+";";return state},
                set:function(value){log+="last-set:"+__show(value)+";";state=value}
            });
            function result(label,index,matched,capture,groups){
                var value=Object();
                Object.defineProperty(value,"length",{get:function(){
                    log+=label+"-length;";return 2;
                }});
                Object.defineProperty(value,"0",{get:function(){
                    log+=label+"-zero;";return matched;
                }});
                Object.defineProperty(value,"1",{get:function(){
                    log+=label+"-capture;";return capture;
                }});
                Object.defineProperty(value,"index",{get:function(){
                    log+=label+"-index;";return index;
                }});
                Object.defineProperty(value,"groups",{get:function(){
                    log+=label+"-groups;";return groups;
                }});
                return value;
            }
            var first=result("first",0,"a","A",group1),
                second=result("second",1,"b","B",group2);
            Object.defineProperty(receiver,"exec",{get:function(){
                log+="exec-get;";
                return function(){
                    calls++;log+="exec-call:"+calls+";";
                    if(calls===1)return first;
                    if(calls===2)return second;
                    return null;
                };
            }});
            function replacement(match,capture,position,input,groups){
                "use strict";
                log+="replace:"+match+":"+capture+":"+position+":"+input+":"+
                    (groups===group1?"g1":groups===group2?"g2":"bad")+":"+
                    arguments.length+";";
                return "X";
            }
            var output=RegExp.prototype[Symbol.replace].call(receiver,"ab",replacement);
            return [output,state,calls,log].join("|");
        })()
// case: global zero is converted once during collection and again during processing
(function(){
            var log="",receiver=Object(),calls=0,zero=Object(),result=Object();
            receiver.flags="g";receiver.lastIndex=0;
            zero.toString=function(){log+="zero-string;";return "a"};
            Object.defineProperty(result,"0",{get:function(){log+="zero-get;";return zero}});
            result.length=1;result.index=0;result.groups=undefined;
            receiver.exec=function(){calls++;log+="exec:"+calls+";";return calls===1?result:null};
            var output=RegExp.prototype[Symbol.replace].call(receiver,"a","X");
            return [output,calls,log].join("|");
        })()
// LENGTH_CAPTURE_GROUP_CASES
// case: QuickJS applies ToUint32 to result length before capture enumeration
(function(){
            function run(rawLength,template){
                var log="",receiver=Object(),length=Object(),result=Object();
                receiver.flags="";
                length.valueOf=function(){log+="length-value;";return rawLength};
                Object.defineProperty(result,"length",{get:function(){
                    log+="length-get;";return length;
                }});
                result[0]="a";result.index=0;result.groups=undefined;
                Object.defineProperty(result,"1",{get:function(){log+="one;";return "ONE"}});
                Object.defineProperty(result,"2",{get:function(){log+="two;";return "TWO"}});
                receiver.exec=function(){return result};
                return RegExp.prototype[Symbol.replace].call(receiver,"a",template)+":"+log;
            }
            return [
                run(4294967297,"<$1>"),
                run(3.9,"<$1|$2|$3|$01|$20>")
            ].join("|");
        })()
// case: capture conversion precedes groups and named substitutions observe boxed primitives
(function(){
            var log="",receiver=Object(),capture=Object(),result=Object();
            receiver.flags="";
            capture.toString=function(){log+="capture-string;";return "C"};
            result.length=2;result[0]="a";result.index=0;
            Object.defineProperty(result,"1",{get:function(){log+="capture-get;";return capture}});
            Object.defineProperty(result,"groups",{get:function(){
                log+="groups-get;";return "123";
            }});
            receiver.exec=function(){return result};
            var output=RegExp.prototype[Symbol.replace].call(
                receiver,"a","$1:$<length>:$<0>:$<missing>");
            return output+"|"+log;
        })()
// case: functional replacement receives stringified captures raw groups position and input
(function(){
            var log="",receiver=Object(),capture=Object(),groups=Object(),result=Object();
            receiver.flags="";
            capture.toString=function(){log+="capture-string;";return "C"};
            result.length=3;result[0]="a";result.index=0;result[1]=capture;
            result[2]=undefined;result.groups=groups;
            receiver.exec=function(){return result};
            function replacement(match,first,second,position,input,named){
                "use strict";
                log+="call:"+(this===undefined)+":"+match+":"+first+":"+
                    (second===undefined)+":"+position+":"+input+":"+
                    (named===groups)+":"+arguments.length+";";
                return {toString:function(){log+="result-string;";return "X"}};
            }
            var output=RegExp.prototype[Symbol.replace].call(receiver,"a",replacement);
            return output+"|"+log;
        })()
// case: real RegExp captures expand dollar before after match and numeric tokens
(function(){
            var regexp=/(b)(c)?/;
            return [
                regexp[Symbol.replace]("abcd","$$|$&|$`|$'|$1|$2|$3|$01|$20"),
                /(a)|(b)/g[Symbol.replace]("ab","<$1:$2>"),
                /(z)?/[Symbol.replace]("a","<$1>")
            ].join("|");
        })()
// case: functional apply limit is checked after every capture and groups observation
(function(){
            var log="",receiver=Object(),result=Object(),lastCapture=Object(),
                groups=Object(),calls=0,error;
            receiver.flags="";
            result.length=65533;result[0]="a";result.index=0;
            lastCapture.toString=function(){log+="last-string;";return "LAST"};
            Object.defineProperty(result,"65532",{get:function(){
                log+="last-get;";return lastCapture;
            }});
            Object.defineProperty(result,"groups",{get:function(){
                log+="groups-get;";return groups;
            }});
            receiver.exec=function(){return result};
            function replacement(){
                calls++;log+="BAD-call;";return "X";
            }
            try{
                RegExp.prototype[Symbol.replace].call(receiver,"a",replacement);
                return "missing|"+log+"|"+calls;
            }catch(caught){
                error=caught;
            }
            return [
                error.name,error.message,
                Object.getPrototypeOf(error)===RangeError.prototype,
                log,calls
            ].join("|");
        })()
// POSITION_CASES
// case: backward custom results still perform replacement work before being ignored
(function(){
        var log="",receiver=Object(),calls=0,results=[
            {length:1,0:"0",index:3,groups:undefined},
            {length:1,0:"0",index:1,groups:undefined}
        ];
        receiver.flags="g";receiver.lastIndex=0;
        receiver.exec=function(){
            calls++;log+="exec:"+calls+";";
            return calls<=2?results[calls-1]:null;
        };
        function replacement(match,position,input){
            log+="replace:"+match+":"+position+":"+input+";";
            return position===3?"X":"Y";
        }
        var output=RegExp.prototype[Symbol.replace].call(receiver,"abcde",replacement);
        return [output,calls,log].join("|");
    })()
// LAST_INDEX_CASES
// case: empty global matches advance one UTF-16 unit or one u-v code point
(function(){
            function run(flags,current){
                var log="",receiver=Object(),state=99,calls=0,result=Object();
                receiver.flags=flags;
                Object.defineProperty(receiver,"lastIndex",{
                    get:function(){log+="get:"+__show(state)+";";return state},
                    set:function(value){log+="set:"+__show(value)+";";state=value}
                });
                result.length=1;result[0]="";result.index=0;result.groups=undefined;
                receiver.exec=function(){
                    calls++;log+="exec:"+calls+";";
                    if(calls===1){state=current;return result}
                    return null;
                };
                var input=String.fromCharCode(0xd83d,0xde00,0xd800);
                var output=RegExp.prototype[Symbol.replace].call(receiver,input,"X");
                return flags+":"+__show(state)+":"+calls+":"+__units(output)+":"+log;
            }
            return [
                run("g",0),run("gu",0),run("gv",0),
                run("gu",1),run("gu",2)
            ].join("|");
        })()
// case: builtin global and sticky replacements preserve pinned lastIndex transitions
(function(){
            var global=/a/g,stickyHit=/a/y,stickyMiss=/a/y;
            global.lastIndex=7;stickyHit.lastIndex=1;stickyMiss.lastIndex=0;
            return [
                global[Symbol.replace]("baac","X"),global.lastIndex,
                stickyHit[Symbol.replace]("ba","X"),stickyHit.lastIndex,
                stickyMiss[Symbol.replace]("ba","X"),stickyMiss.lastIndex
            ].join("|");
        })()
// FAST_PATH_CASES
// case: standard-regexp fast path activates only after exec AutoInit materialization
(function(){
            var hits=0,regexp=/a/;
            Object.defineProperty(regexp,"ignoreCase",{get:function(){
                hits++;return false;
            }});
            var first=regexp[Symbol.replace]("a","X"),
                second=regexp[Symbol.replace]("a","Y");
            return [first,second,hits,
                RegExp.prototype.exec===RegExp.prototype.exec].join("|");
        })()
// case: replacement conversion can materialize exec before the same replace predicate
(function(){
            var hits=0,regexp=/a/,replacement=Object();
            Object.defineProperty(regexp,"ignoreCase",{get:function(){
                hits++;return false;
            }});
            replacement.toString=function(){
                RegExp.prototype.exec;
                return "X";
            };
            return regexp[Symbol.replace]("a",replacement)+"|"+hits;
        })()
// case: required raw slots and exotic prototypes force the generic path
(function(){
            RegExp.prototype.exec;
            function run(kind){
                var log="",regexp=/a/,builtin=RegExp.prototype.exec;
                Object.defineProperty(regexp,"ignoreCase",{get:function(){
                    log+="i";return false;
                }});
                if(kind==="exec"){
                    regexp.exec=function(value){
                        log+="e";return builtin.call(this,value);
                    };
                }else if(kind==="lastIndex"){
                    var marker=Object();
                    marker.valueOf=function(){log+="l";return 0};
                    regexp.lastIndex=marker;
                }else if(kind==="flags"){
                    Object.defineProperty(regexp,"flags",{get:function(){
                        log+="f";return "";
                    }});
                }else if(kind==="global"){
                    Object.defineProperty(regexp,"global",{get:function(){
                        log+="g";return false;
                    }});
                }else if(kind==="unicode"){
                    Object.defineProperty(regexp,"unicode",{get:function(){
                        log+="u";return false;
                    }});
                }else{
                    var exotic=[];
                    Object.setPrototypeOf(exotic,RegExp.prototype);
                    Object.setPrototypeOf(regexp,exotic);
                }
                return kind+":"+regexp[Symbol.replace]("a","X")+":"+log;
            }
            return [
                run("exec"),run("lastIndex"),run("flags"),
                run("global"),run("unicode"),run("exotic")
            ].join("|");
        })()
// case: unchecked flag getters and unrelated own properties stay off the direct path surface
(function(){
            RegExp.prototype.exec;
            var log="",regexp=/a/;
            regexp.extra=1;
            regexp.exec=RegExp.prototype.exec;
            Object.defineProperty(regexp,"ignoreCase",{get:function(){
                log+="i";return false;
            }});
            Object.defineProperty(regexp,"multiline",{get:function(){
                log+="m";return false;
            }});
            Object.defineProperty(regexp,"dotAll",{get:function(){
                log+="s";return false;
            }});
            Object.defineProperty(regexp,"sticky",{get:function(){
                log+="y";return false;
            }});
            Object.defineProperty(regexp,"unicodeSets",{get:function(){
                log+="v";return false;
            }});
            Object.defineProperty(regexp,"hasIndices",{get:function(){
                log+="d";return false;
            }});
            var direct=regexp[Symbol.replace]("a","X");

            var functional=/a/;
            Object.defineProperty(functional,"ignoreCase",{get:function(){
                log+="f";return false;
            }});
            var generic=functional[Symbol.replace]("a",function(){return "Z"});
            return [direct,regexp.extra,log,generic].join("|");
        })()
// case: prewarmed direct matcher preserves captures global sticky and Unicode advancement
(function(){
            RegExp.prototype.exec;
            var captures=/(b)(c)?/,plain=/a/,global=/a/g,
                sticky=/a/y,stickyMiss=/a/y,
                astral=String.fromCharCode(0xd83d,0xde00,0xd800),
                emptyUnicode=new RegExp("","gu");
            plain.lastIndex=7;
            global.lastIndex=7;
            sticky.lastIndex=1;
            stickyMiss.lastIndex=0;
            return [
                captures[Symbol.replace](
                    "abcd","$$|$&|$`|$'|$1|$2|$3|$01|$20"),
                plain[Symbol.replace]("a","X"),plain.lastIndex,
                global[Symbol.replace]("baac","X"),global.lastIndex,
                sticky[Symbol.replace]("ba","X"),sticky.lastIndex,
                stickyMiss[Symbol.replace]("ba","X"),stickyMiss.lastIndex,
                __units(emptyUnicode[Symbol.replace](astral,"-")),
                emptyUnicode.lastIndex
            ].join("|");
        })()
// case: direct matcher writes only the lastIndex modes required by QuickJS
(function(){
            RegExp.prototype.exec;
            function locked(flags,lastIndex,input){
                var regexp=new RegExp("a",flags);
                regexp.lastIndex=lastIndex;
                Object.defineProperty(regexp,"lastIndex",{writable:false});
                return __completion(function(){
                    return regexp[Symbol.replace](input,"X");
                })+":"+regexp.lastIndex;
            }
            return [
                locked("",7,"a"),
                locked("g",7,"a"),
                locked("y",0,"a"),
                locked("y",1,"ba")
            ].join("|");
        })()
