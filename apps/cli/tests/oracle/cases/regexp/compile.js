// prelude: regexp_completion
// METADATA_CASES
// case: RegExp compile exposes pinned metadata descriptor and source-table order
(function(){
            var fn=RegExp.prototype.compile,
                keys=Reflect.ownKeys(RegExp.prototype),
                selected=[],index=0,key;
            while(index<keys.length){
                key=keys[index++];
                if(key==="exec"||key==="compile"||key==="test"||key==="toString"||
                    key==="constructor"||key===Symbol.match||key===Symbol.search||
                    key===Symbol.split)
                    selected[selected.length]=String(key);
            }
            return [
                selected.join(","),
                __bits(RegExp.prototype,"compile"),fn.name,fn.length,
                Object.getOwnPropertyNames(fn).join(","),
                __bits(fn,"name"),__bits(fn,"length"),
                __isConstructor(fn),
                Object.prototype.hasOwnProperty.call(fn,"prototype")
            ].join("|");
        })()
// case: RegExp compile AutoInit identity is stable and independently replaceable
(function(){
            var first=RegExp.prototype.compile,
                second=RegExp.prototype.compile,
                stable=first===second,
                deleted=delete RegExp.prototype.compile;
            RegExp.prototype.compile=17;
            return [stable,deleted,RegExp.prototype.compile,
                __bits(RegExp.prototype,"compile"),
                typeof first,first.name,first.length].join("|");
        })()
// BRAND_AND_COPY_CASES
// case: RegExp compile validates the receiver before touching the arguments
(function(){
            var log="",pattern=Object(),compile=RegExp.prototype.compile;
            pattern.toString=function(){log+="BAD";throw "converted"};
            var receivers=[undefined,null,23,true,"text",Object(),[]],output=[],index=0;
            while(index<receivers.length){
                output[output.length]=__completion(function(receiver){
                    return function(){return compile.call(receiver,pattern)};
                }(receivers[index++]));
            }
            return output.join(",")+"|"+log;
        })()
// case: RegExp compile copies genuine same and distinct patterns without public property reads
(function(){
            var log="",subject=/old/g,pattern=/new/imy;
            subject.lastIndex=23;pattern.lastIndex=45;
            Object.defineProperty(pattern,"source",{get:function(){log+="BAD-source;";throw 1}});
            Object.defineProperty(pattern,"flags",{get:function(){log+="BAD-flags;";throw 2}});
            Object.defineProperty(pattern,Symbol.match,{get:function(){
                log+="BAD-match;";throw 3;
            }});
            var result=subject.compile(pattern),
                distinct=[result===subject,subject.source,subject.flags,
                    subject.lastIndex,pattern.lastIndex,log].join(":");
            subject.lastIndex=17;
            result=subject.compile(subject);
            return distinct+"|"+
                [result===subject,subject.source,subject.flags,subject.lastIndex,log].join(":");
        })()
// case: RegExp compile rejects defined flags for genuine patterns before coercion
(function(){
            var log="",subject=/old/g,pattern=/new/i,flags=Object();
            subject.lastIndex=29;
            flags.toString=function(){log+="BAD-flags;";throw "converted"};
            var result=__completion(function(){return subject.compile(pattern,flags)});
            return [result,log,subject.source,subject.flags,subject.lastIndex,
                pattern.source,pattern.flags].join("|");
        })()
// case: RegExp compile accepts a genuine derived instance like pinned QuickJS
(function(){
            function Derived(){}
            Derived.prototype=Object.create(RegExp.prototype);
            Object.defineProperty(Derived.prototype,"constructor",{
                value:Derived,writable:true,configurable:true
            });
            var regexp=Reflect.construct(RegExp,["before","g"],Derived);
            regexp.lastIndex=7;
            var result=regexp.compile("after","i");
            return [result===regexp,regexp instanceof Derived,
                Object.getPrototypeOf(regexp)===Derived.prototype,
                regexp.source,regexp.flags,regexp.lastIndex,regexp.test("AFTER")].join("|");
        })()
// CONVERSION_CASES
// case: RegExp compile converts pattern before flags then replaces matcher state
(function(){
            var log="",subject=/old/g,pattern=Object(),flags=Object();
            subject.lastIndex=31;
            pattern.toString=function(){log+="pattern;";return "n(e)w"};
            flags.toString=function(){log+="flags;";return "im"};
            var result=subject.compile(pattern,flags);
            return [result===subject,log,subject.source,subject.flags,subject.lastIndex,
                subject.test("NEW"),subject.exec("new")[1]].join("|");
        })()
// case: RegExp compile maps undefined pattern and flags to empty strings
(function(){
            var implicit=/old/g,explicit=/old/i,a,b;
            implicit.lastIndex=9;explicit.lastIndex=11;
            a=implicit.compile();
            b=explicit.compile(undefined,undefined);
            return [a===implicit,implicit.source,implicit.flags,implicit.lastIndex,
                implicit.test(""),b===explicit,explicit.source,explicit.flags,
                explicit.lastIndex,explicit.test("")].join("|");
        })()
// case: RegExp compile preserves canonical flags and exact source spelling
(function(){
            var regexp=/old/;
            regexp.compile("a/b\nc\r","ydgimsu");
            return [regexp.source,regexp.flags,regexp.hasIndices,regexp.global,
                regexp.ignoreCase,regexp.multiline,regexp.dotAll,regexp.unicode,
                regexp.sticky].join("|");
        })()
// FAILURE_ATOMICITY_CASES
// case: RegExp compile conversion failures leave source matcher and lastIndex unchanged
(function(){
            var log="",subject=/stable/gi,pattern=Object(),flags=Object(),first,second;
            subject.lastIndex=19;
            pattern.toString=function(){log+="pattern-throw;";throw "pattern-boom"};
            flags.toString=function(){log+="BAD-flags;";return "m"};
            first=__completion(function(){return subject.compile(pattern,flags)});
            var afterPattern=[subject.source,subject.flags,subject.lastIndex,
                subject.test("STABLE")].join(":");
            subject.lastIndex=19;
            pattern.toString=function(){log+="pattern-ok;";return "changed"};
            flags.toString=function(){log+="flags-throw;";throw "flags-boom"};
            second=__completion(function(){return subject.compile(pattern,flags)});
            return [first,afterPattern,second,subject.source,subject.flags,
                subject.lastIndex,subject.test("STABLE"),log].join("|");
        })()
// case: RegExp compile syntax and flag failures are atomic
(function(){
            var subject=/stable/gi,output=[];
            function attempt(pattern,flags){
                subject.lastIndex=27;
                output[output.length]=__completion(function(){
                    return subject.compile(pattern,flags);
                });
                output[output.length]=
                    [subject.source,subject.flags,subject.lastIndex,
                        subject.test("STABLE")].join(":");
            }
            attempt("?","");
            attempt("changed","gg");
            attempt("changed","uv");
            attempt("\\2","u");
            return output.join("|");
        })()
// case: RegExp compile rejects Symbol conversion without mutating the target
(function(){
            var subject=/stable/g,output=[];
            subject.lastIndex=13;
            output[output.length]=__completion(function(){
                return subject.compile(Symbol("pattern"));
            });
            output[output.length]=
                [subject.source,subject.flags,subject.lastIndex].join(":");
            output[output.length]=__completion(function(){
                return subject.compile("changed",Symbol("flags"));
            });
            output[output.length]=
                [subject.source,subject.flags,subject.lastIndex].join(":");
            return output.join("|");
        })()
// case: RegExp compile reentrant conversions retain inner effects until an outer commit
(function(){
            var success=/start/g,patternFailure=/start/g,flagsFailure=/start/g,log="";
            success.lastIndex=5;
            var result=success.compile({toString:function(){
                success.compile("inner","i");log+="success-inner;";return "outer";
            }},"m");
            patternFailure.lastIndex=7;
            var patternCompletion=__completion(function(){
                return patternFailure.compile({toString:function(){
                    patternFailure.compile("inner","i");
                    log+="pattern-inner;";return "[";
                }});
            });
            flagsFailure.lastIndex=9;
            var flags=Object();
            flags.toString=function(){
                flagsFailure.compile("flag-inner","y");
                log+="flags-inner;";throw "flags-boom";
            };
            var flagsCompletion=__completion(function(){
                return flagsFailure.compile("outer",flags);
            });
            return [result===success,success.source,success.flags,success.lastIndex,
                patternCompletion,patternFailure.source,patternFailure.flags,
                patternFailure.lastIndex,flagsCompletion,flagsFailure.source,
                flagsFailure.flags,flagsFailure.lastIndex,log].join("|");
        })()
// MUTATION_ORDER_CASES
// case: RegExp compile replaces internal state before readonly lastIndex throws
(function(){
            var subject=/initial/;
            Object.defineProperty(subject,"lastIndex",{value:45,writable:false});
            var completion=__completion(function(){return subject.compile(/updated/gi)});
            return [completion,subject.source,subject.flags,subject.lastIndex,
                new RegExp(subject).test("UPDATED")].join("|");
        })()
// case: RegExp compile resets only the receiver lastIndex and returns that receiver
(function(){
            var subject=/subject/g,pattern=/pattern/y;
            subject.lastIndex=8;pattern.lastIndex=6;
            var result=RegExp.prototype.compile.call(subject,pattern);
            return [result===subject,result===pattern,subject.source,subject.flags,
                subject.lastIndex,pattern.source,pattern.flags,pattern.lastIndex].join("|");
        })()
