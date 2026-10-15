// DIRECT_CASES
// case: direct object and array catch bindings
(function(){
            var objectValue,arrayValue;
            try{throw {left:40,right:2}}catch({left,right}){objectValue=left+right}
            try{throw [6,7]}catch([first,second]){arrayValue=first*second}
            return objectValue+'|'+arrayValue;
        })()
// case: recursive object and array patterns share defaults and elisions
(function(){
            try{throw {left:[0,{value:undefined}],right:{items:[41,0,42]}}}
            catch({left:[,{value=40}],right:{items:[first,,last]}}){
                return value+'|'+first+'|'+last;
            }
        })()
// case: leaf defaults distinguish undefined from null and zero
(function(){
            try{throw {missing:undefined,nil:null,zero:0}}
            catch({missing=40,nil=41,zero=42}){
                return missing+'|'+nil+'|'+zero;
            }
        })()
// case: whole object and array catch patterns accept defaults
(function(){
            var objectResult,arrayResult;
            try{throw undefined}catch({value}={value:40}){objectResult=value}
            try{throw undefined}catch([value]=[42]){arrayResult=value}
            return objectResult+'|'+arrayResult;
        })()
// case: object catch binding boxes strings after its nullish guard
(function(){
            var boxed;
            try{throw 'abc'}catch({0:first,length}){boxed=first+'|'+length}
            try{try{throw null}catch({missing}){}}
            catch(error){return boxed+'|'+error.name}
        })()
// case: array catch binding rejects a non iterable object
(function(){
            try{try{throw {value:42}}catch([value]){}}
            catch(error){return error.name+'|'+error.message}
        })()
// REST_CASES
// case: object rest excludes fixed computed and Symbol keys
(function(){
            var excluded=Symbol('excluded'),kept=Symbol('kept'),source={fixed:1,other:40};
            source[excluded]=2;source[kept]=42;
            try{throw source}
            catch({fixed,[excluded]:symbolic,...rest}){
                var symbols=Object.getOwnPropertySymbols(rest);
                return fixed+'|'+symbolic+'|'+rest.other+'|'+rest[kept]+'|'+
                    Object.hasOwn(rest,'fixed')+'|'+Object.hasOwn(rest,excluded)+'|'+
                    (symbols.length===1&&symbols[0]===kept)+'|'+
                    (Object.getPrototypeOf(rest)===Object.prototype);
            }
        })()
// case: object rest copies enumerable own properties and skips inherited and hidden keys
(function(){
            var source=Object.create({inherited:1});
            source.fixed=2;source.kept=40;
            Object.defineProperty(source,'hidden',{value:3,enumerable:false});
            try{throw source}catch({fixed,...rest}){
                return fixed+'|'+rest.kept+'|'+Object.keys(rest).join(',')+'|'+
                    ('inherited' in rest)+'|'+('hidden' in rest);
            }
        })()
// case: array rest drains the iterator without an early close
(function(){
            var log='',index=0,iterator={
                [Symbol.iterator]:function(){log+='open|';return this},
                next:function(){log+='next'+index+'|';return index<3?
                    {value:40+index++,done:false}:{done:true}},
                return:function(){log+='close|';return {done:true}}
            };
            try{throw iterator}catch([first,...rest]){
                return first+'|'+rest.join(',')+'|'+log;
            }
        })()
// case: rest recursively composes object and array catch patterns
(function(){
            try{throw {outer:{fixed:1,left:40,right:2},items:[3,{skip:4,deep:42}]}}
            catch({outer:{fixed,...objectRest},items:[head,{skip,...deepRest}]}){
                return fixed+'|'+objectRest.left+'|'+objectRest.right+'|'+
                    head+'|'+skip+'|'+deepRest.deep;
            }
        })()
// ORDER_AND_NAME_CASES
// case: computed key converts once before Get and default
(function(){
            var log='',key={},source={};
            key[Symbol.toPrimitive]=function(hint){log+='key:'+hint+'|';return 'value'};
            Object.defineProperty(source,'value',{get:function(){log+='get|';return undefined}});
            try{throw source}catch({[key]:value=(log+='default|',42)}){
                return value+'|'+log;
            }
        })()
// case: object conversion precedes computed key evaluation
(function(){
            var log='',key={};
            key[Symbol.toPrimitive]=function(){log+='key|';return 'value'};
            try{try{throw undefined}catch({[key]:value}){}}
            catch(error){return error.name+'|'+log}
        })()
// case: computed properties evaluate key Get and default from left to right
(function(){
            var log='',first={},second={},source={};
            first[Symbol.toPrimitive]=function(){log+='key1|';return 'a'};
            second[Symbol.toPrimitive]=function(){log+='key2|';return 'b'};
            Object.defineProperty(source,'a',{get:function(){log+='get1|';return 40}});
            Object.defineProperty(source,'b',{get:function(){log+='get2|';return undefined}});
            try{throw source}catch({[first]:a=(log+='default1|',1),[second]:b=(log+='default2|',2)}){
                return a+'|'+b+'|'+log;
            }
        })()
// case: computed exclusions are reused by object rest
(function(){
            var calls=0,key={},source={fixed:40,kept:2};
            key[Symbol.toPrimitive]=function(){calls++;return 'fixed'};
            try{throw source}catch({[key]:value,...rest}){
                return calls+'|'+value+'|'+rest.kept+'|'+Object.hasOwn(rest,'fixed');
            }
        })()
// case: anonymous functions in object and array leaf defaults receive names
(function(){
            var objectName,arrayName,nestedName;
            try{throw {}}
            catch({objectFn=function(){},nested:{nestedFn=function(){}}={}}){
                objectName=objectFn.name;nestedName=nestedFn.name;
            }
            try{throw []}catch([arrayFn=function(){}]){arrayName=arrayFn.name}
            return objectName+'|'+arrayName+'|'+nestedName;
        })()
// SCOPE_AND_EVAL_CASES
// case: catch pattern bindings are mutable lexical cells
(function(){try{throw {value:40}}catch({value}){value+=2;return value}})()
// case: pattern initializer closure keeps the parameter cell across body shadowing
(function(){
            try{throw {read:undefined,value:7}}
            catch({read=function(){return value},value}){
                let value=9;
                let bodyRead=function(){return value};
                return read.name+'|'+read()+'|'+bodyRead();
            }
        })()
// case: pattern catch permits a same name body lexical unlike simple catch
(function(){try{throw {value:1}}catch({value}){let value=42;return value}})()
// case: reentered pattern catch creates fresh captured cells
(function(){
            var first,second,index=0;
            while(index<2){
                try{throw {value:++index}}
                catch({value}){
                    if(index===1)first=function(){return value};
                    else second=function(){return value};
                }
            }
            return first()+'|'+second()+'|'+(first()!==second());
        })()
// case: direct eval reads and writes a pattern binding
(function(){try{throw {value:40}}catch({value}){eval('value+=2');return eval('value')}})()
// case: direct eval var reuses a simple catch binding
(function(){try{throw 1}catch(value){eval('var value=42');return value}})()
// case: direct eval var cannot redeclare a pattern catch binding
(function(){
            try{throw {value:1}}
            catch({value}){
                try{eval('var value=2');return 'missing'}
                catch(error){return error.name+'|'+error.message}
            }
        })()
// case: simple catch var reuses its binding
(function(){try{throw 1}catch(value){var value=42;return value}})()
// ITERATOR_CASES
// case: one element array pattern closes an unfinished iterator
(function(){
            var log='',iterator={
                [Symbol.iterator]:function(){log+='open|';return this},
                next:function(){log+='next|';return {value:40,done:false}},
                return:function(){log+='close|';return {done:true}}
            };
            try{throw iterator}catch([value]){return value+'|'+log}
        })()
// case: empty array pattern opens then closes without calling next
(function(){
            var log='',iterator={
                [Symbol.iterator]:function(){log+='open|';return this},
                next:function(){log+='next|';return {value:40,done:false}},
                return:function(){log+='close|';return {done:true}}
            };
            try{throw iterator}catch([]){return log}
        })()
// case: default fault closes the iterator and wins over close fault
(function(){
            var log='',iterator={
                [Symbol.iterator]:function(){log+='open|';return this},
                next:function(){log+='next|';return {value:undefined,done:false}},
                return:function(){log+='close|';throw 'close-error'}
            };
            try{try{throw iterator}
                catch([value=(log+='default|',function(){throw 'default-error'}())]){}}
            catch(error){return log+'caught:'+error}
        })()
// case: nested iterator faults close inner then outer and keep the original fault
(function(){
            var log='',inner={
                [Symbol.iterator]:function(){log+='inner-open|';return this},
                next:function(){log+='inner-next|';return {value:undefined,done:false}},
                return:function(){log+='inner-close|';throw 'inner-close-error'}
            },outer={
                [Symbol.iterator]:function(){log+='outer-open|';return this},
                next:function(){log+='outer-next|';return {value:inner,done:false}},
                return:function(){log+='outer-close|';throw 'outer-close-error'}
            };
            try{try{throw outer}
                catch([[value=(log+='default|',function(){throw 'default-error'}())]]){}}
            catch(error){return log+'caught:'+error}
        })()
// case: iterator next fault does not call return
(function(){
            var log='',iterator={
                [Symbol.iterator]:function(){log+='open|';return this},
                next:function(){log+='next|';throw 'next-error'},
                return:function(){log+='close|';return {done:true}}
            };
            try{try{throw iterator}catch([value]){}}
            catch(error){return log+'caught:'+error}
        })()
// PINNED_QUIRK_CASES
// case: object catch pattern skips TDZ initialization
(function(){try{throw {}}catch({first=second,second}){return String(first)+'|'+String(second)}})()
// case: array catch pattern skips TDZ initialization
(function(){try{throw []}catch([first=second,second]){return String(first)+'|'+String(second)}})()
// case: computed key observes a later uninitialized binding as undefined
(function(){try{throw {undefined:9}}catch({[later]:value,later}){return value+'|'+String(later)}})()
// case: whole catch patterns accept top level defaults
(function(){var a,b;try{throw undefined}catch({value}={value:40}){a=value}try{throw undefined}catch([value]=[42]){b=value}return a+'|'+b})()
// case: abrupt catch initialization closes its iterator but skips its own finally
(function(){
            var log='',iterator={
                [Symbol.iterator]:function(){log+='open|';return this},
                next:function(){log+='next|';return {value:undefined,done:false}},
                return:function(){log+='close|';return {done:true}}
            };
            try{
                try{throw iterator}
                catch([value=(log+='default|',null.value)]){log+='body|'}
                finally{log+='finally|'}
            }catch(error){return log+error.name}
        })()
