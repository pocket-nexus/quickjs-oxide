// DIRECT_CASES
// case: fixed shorthand string number and keyword property names
(function(){
            var {fixed: a, shorthand, missing: defaulted=41}={fixed:1,shorthand:2};
            let {"text": text, 1: one}={text:3,1:4};
            const {if: keyword}={if:5};
            return a+'|'+shorthand+'|'+defaulted+'|'+text+'|'+one+'|'+keyword;
        })()
// case: computed string number and Symbol keys
(function(){
            var symbol=Symbol('object-binding'),source={text:40,1:1};
            source[symbol]=2;
            const {['text']: text,[1]: one,[symbol]: symbolic}=source;
            return text+one+symbolic;
        })()
// case: defaults use strict undefined and anonymous functions receive names
(function(){
            let {zero=9,nil=8,missing=7,named=function(){}}={zero:0,nil:null};
            return zero+'|'+nil+'|'+missing+'|'+named.name;
        })()
// case: recursive object and array patterns share one source value
(function(){
            let {left:{x=40}={},right:[y=41,{z=42}]=[]}={left:{},right:[undefined,{}]};
            const [{a},{b:[c]}]=[{a:1},{b:[2]}];
            return x+'|'+y+'|'+z+'|'+a+'|'+c;
        })()
// case: nested pattern defaults run before recursive object conversion
(function(){
            var log='';
            let {outer:{value=(log+='V',40)}=(log+='P',{})}={outer:undefined};
            return value+'|'+log;
        })()
// case: object rest excludes fixed computed and Symbol keys
(function(){
            var symbol=Symbol('excluded'),source={fixed:1,kept:2};source[symbol]=3;
            const {fixed,[symbol]:symbolic,...rest}=source;
            return fixed+'|'+symbolic+'|'+rest.kept+'|'+('fixed' in rest)+'|'+(symbol in rest);
        })()
// SURFACE_CASES
// case: classic for var let and const object-binding heads
(function(){
            var log='';
            for(var {x}={x:1};x<3;x++)log+='v'+x;
            for(let {y=3}={};y<5;y++)log+='l'+y;
            for(const {z}={z:5};z===5;) { log+='c'+z; break; }
            return log;
        })()
// case: classic for computed key accepts in inside its NoIn initializer
(function(){
            var result;
            for(let {[('x' in {x:1})?'x':'y']: value}={x:42};(result=value,false);){}
            return result;
        })()
// case: for-of var binding keeps the final property value
(function(){
            var log='';
            for(var {x=0} of [{x:1},{x:2},{}])log+=x;
            return log+'|'+x;
        })()
// case: for-of lexical object bindings receive fresh captured cells
(function(){
            var first,second,index=0;
            for(let {x=40} of [{x:1},{}]) {
                if(index++===0)first=function(){return x};
                else second=function(){return x};
            }
            return first()+'|'+second();
        })()
// case: for-of const binding supports computed Symbol keys and nested arrays
(function(){
            var key=Symbol('key'),source={};source[key]=[40,2];
            var result='';
            for(const {[key]:[left,right]} of [source])result=left+'|'+right;
            return result;
        })()
// case: for-in object binding consumes each yielded string key
(function(){
            var result='';
            for(const {0:first,1:second} in {ab:1,cd:2})result+=first+second;
            return result;
        })()
// ORDER_CASES
// case: computed key ToPropertyKey runs once with string hint before get and default
(function(){
            var log='',key={};
            key[Symbol.toPrimitive]=function(hint){log+='K:'+hint+'|';return 'x'};
            var source={};
            Object.defineProperty(source,'x',{get:function(){log+='G|';return undefined}});
            var {[key]: value=(log+='D|',42)}=source;
            return value+'|'+log;
        })()
// case: object conversion precedes computed property evaluation
(function(){
            var log='',key={};
            key[Symbol.toPrimitive]=function(){log+='K';return 'x'};
            try{let {[key]: value}=null}catch(error){return error.name+'|'+log}
        })()
// case: properties evaluate key get and default from left to right
(function(){
            var log='',first={},second={},source={};
            first[Symbol.toPrimitive]=function(hint){log+='K1:'+hint+'|';return 'a'};
            second[Symbol.toPrimitive]=function(hint){log+='K2:'+hint+'|';return 'b'};
            Object.defineProperty(source,'a',{get:function(){log+='Ga|';return 1}});
            Object.defineProperty(source,'b',{get:function(){log+='Gb|';return undefined}});
            Object.defineProperty(source,'z',{get:function(){log+='Gz|';return 3}});
            let {[first]: x=(log+='Da|',8),[second]: y=(log+='Db|',9),z}=source;
            return x+'|'+y+'|'+z+'|'+log;
        })()
// case: fixed sloppy var prepares its with reference before the property getter
(function(){
            var value='outer',scope={value:'scope'},source={};
            Object.defineProperty(source,'x',{get:function(){delete scope.value;return 7}});
            with(scope){var {x:value}=source}
            return value+'|'+scope.value;
        })()
// case: computed sloppy var evaluates key then prepares with reference before get
(function(){
            var value='outer',scope={},source={},key={};
            key[Symbol.toPrimitive]=function(hint){scope.value='scope';return 'x'};
            Object.defineProperty(source,'x',{get:function(){delete scope.value;return 8}});
            with(scope){var {[key]:value}=source}
            return value+'|'+scope.value;
        })()
// case: nested var prepares its leaf reference after the outer get and before the inner get
(function(){
            var value='outer',scope={},inner={},source={};
            Object.defineProperty(source,'outer',{get:function(){scope.value='scope';return inner}});
            Object.defineProperty(inner,'inner',{get:function(){delete scope.value;return 9}});
            with(scope){var {outer:{inner:value}}=source}
            return value+'|'+scope.value;
        })()
// FOR_OF_FAULT_CASES
// case: computed key fault closes the outer iterator
(function(){
            var log='',done=false,key={};
            key[Symbol.toPrimitive]=function(){log+='K|';throw 'key-error'};
            var outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(done)return{done:true};done=true;return{value:{x:1},done:false}},
                return:function(){log+='OC|';return{done:true}}
            };
            try{for(const {[key]:value} of outer){}}
            catch(error){return log+'C:'+error}
        })()
// case: property getter fault closes the outer iterator
(function(){
            var log='',done=false,source={};
            Object.defineProperty(source,'x',{get:function(){log+='G|';throw 'get-error'}});
            var outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(done)return{done:true};done=true;return{value:source,done:false}},
                return:function(){log+='OC|';throw 'outer-close'}
            };
            try{for(const {x} of outer){}}
            catch(error){return log+'C:'+error}
        })()
// case: nested array close fault closes outer and keeps the inner fault
(function(){
            var log='',outerDone=false,inner={
                [Symbol.iterator]:function(){log+='II|';return this},
                next:function(){log+='IN|';return{value:1,done:false}},
                return:function(){log+='IC|';throw 'inner-close'}
            },source={};
            Object.defineProperty(source,'x',{get:function(){log+='G|';return inner}});
            var outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(outerDone)return{done:true};outerDone=true;return{value:source,done:false}},
                return:function(){log+='OC|';throw 'outer-close'}
            };
            try{for(const {x:[value]} of outer){}}
            catch(error){return log+'C:'+error}
        })()
// case: nested array next fault skips inner close but closes outer
(function(){
            var log='',outerDone=false,inner={
                [Symbol.iterator]:function(){log+='II|';return this},
                next:function(){log+='IN|';throw 'inner-next'},
                return:function(){log+='IC|';return{done:true}}
            },source={};
            Object.defineProperty(source,'x',{get:function(){log+='G|';return inner}});
            var outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(outerDone)return{done:true};outerDone=true;return{value:source,done:false}},
                return:function(){log+='OC|';throw 'outer-close'}
            };
            try{for(const {x:[value]} of outer){}}
            catch(error){return log+'C:'+error}
        })()
// case: nested array default fault closes inner then outer and keeps the default fault
(function(){
            var log='',outerDone=false,inner={
                [Symbol.iterator]:function(){log+='II|';return this},
                next:function(){log+='IN|';return{value:undefined,done:false}},
                return:function(){log+='IC|';throw 'inner-close'}
            },source={};
            Object.defineProperty(source,'x',{get:function(){log+='G|';return inner}});
            var outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(outerDone)return{done:true};outerDone=true;return{value:source,done:false}},
                return:function(){log+='OC|';throw 'outer-close'}
            };
            try{for(const {x:[value=(log+='D|',function(){throw 'default-error'})()]} of outer){}}
            catch(error){return log+'C:'+error}
        })()
// case: nested object conversion fault closes the outer iterator
(function(){
            var log='',done=false,source={};
            Object.defineProperty(source,'x',{get:function(){log+='G|';return null}});
            var outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(done)return{done:true};done=true;return{value:source,done:false}},
                return:function(){log+='OC|';return{done:true}}
            };
            try{for(const {x:{value}} of outer){}}
            catch(error){return log+'C:'+error.name}
        })()
// case: binding Put fault closes the outer iterator and keeps the Put fault
(function(){
            var log='',done=false,scope={},source={};
            Object.defineProperty(scope,'value',{set:function(){log+='S|';throw 'put-error'}});
            Object.defineProperty(source,'x',{get:function(){log+='G|';return 1}});
            var outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(done)return{done:true};done=true;return{value:source,done:false}},
                return:function(){log+='OC|';throw 'outer-close'}
            };
            try{with(scope){for(var {x:value} of outer){}}}
            catch(error){return log+'C:'+error}
        })()
