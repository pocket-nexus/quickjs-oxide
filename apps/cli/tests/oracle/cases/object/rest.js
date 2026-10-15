// DIRECT_CASES
// case: var let and const rest exclude fixed numeric and Symbol keys
(function(){
            var excluded=Symbol('excluded'),kept=Symbol('kept'),proto={inherited:'I'};
            var source=Object.create(proto),hidden={value:'H',enumerable:false,configurable:true};
            source[2]='two';source.fixed='F';source.keep='K';source[excluded]='X';source[kept]='S';
            Object.defineProperty(source,'hidden',hidden);
            var {fixed,2:numeric,[excluded]:symbolic,...varRest}=source;
            let {keep,...letRest}=source;
            const {missing=42,...constRest}=source;
            var varSymbols=Object.getOwnPropertySymbols(varRest);
            var letSymbols=Object.getOwnPropertySymbols(letRest);
            var descriptor=Object.getOwnPropertyDescriptor(varRest,'keep');
            return fixed+':'+numeric+':'+symbolic+'|'+Object.getOwnPropertyNames(varRest).join(',')+
                ':'+varRest.keep+':'+(varSymbols.length===1&&varSymbols[0]===kept)+':'+varRest[kept]+'|'+
                Object.getOwnPropertyNames(letRest).join(',')+':'+
                (letSymbols.length===2&&letSymbols[0]===excluded&&letSymbols[1]===kept)+'|'+
                missing+':'+Object.getOwnPropertyNames(constRest).join(',')+'|'+
                descriptor.writable+':'+descriptor.enumerable+':'+descriptor.configurable+':'+
                (Object.getPrototypeOf(varRest)===Object.prototype)+':'+
                (varRest.inherited===undefined)+':'+(varRest.hidden===undefined);
        })()
// case: rest recursively nests through object and array binding patterns
(function(){
            var source={
                outer:{fixed:1,left:40,right:2},
                list:[3,{fixed:4,deep:41,extra:5}],
                top:42
            };
            let {
                outer:{fixed,...inner},
                list:[head,{fixed:otherFixed,...deep}],
                ...top
            }=source;
            return fixed+':'+Object.keys(inner).join(',')+':'+inner.left+':'+inner.right+'|'+
                head+':'+otherFixed+':'+Object.keys(deep).join(',')+':'+deep.deep+':'+deep.extra+'|'+
                Object.keys(top).join(',')+':'+top.top;
        })()
// case: object rest boxes primitive binding sources after the nullish guard
(function(){
            let {0:first,...stringRest}='abc';
            const {...numberRest}=17;
            var nullLog='';
            try{let {...missing}=null}catch(error){nullLog=error.name}
            return first+'|'+Object.keys(stringRest).join(',')+':'+stringRest[1]+stringRest[2]+'|'+
                Object.keys(numberRest).length+'|'+nullLog;
        })()
// LOOP_CASES
// case: classic for accepts var let and const object-rest initializers
(function(){
            var log='';
            for(var {x,...rest}={x:1,y:2};x<2;x++)log+='v'+rest.y;
            for(let {x,...rest}={x:2,y:3};x<3;x++)log+='l'+rest.y;
            for(const {x,...rest}={x:4,y:5};x===4;){log+='c'+rest.y;break}
            return log;
        })()
// case: for-of var rest keeps the final fresh result object
(function(){
            var log='';
            for(var {x,...rest} of [{x:1,y:40},{x:2,y:41}])log+=x+':'+rest.y+'|';
            return log+'last:'+x+':'+rest.y;
        })()
// case: for-of lexical rest bindings receive fresh captured cells and objects
(function(){
            var closures=[],rests=[];
            for(let {x,...rest} of [{x:1,y:40},{x:2,y:41}]){
                rests.push(rest);closures.push(function(){return x+':'+rest.y});rest.y+=1;
            }
            var symbol=Symbol('loop'),source={fixed:3};source[symbol]=42;
            var symbolic;
            for(const {fixed,...rest} of [source])symbolic=rest[symbol];
            return closures[0]()+'|'+closures[1]()+'|'+(rests[0]!==rests[1])+'|'+symbolic;
        })()
// case: for-in rest copies the remaining UTF-16 index of each yielded key
(function(){
            var log='';
            for(const {0:first,...rest} in {ab:1,cd:2})log+=first+rest[1]+'|';
            return log;
        })()
// EXCLUSION_AND_ORDER_CASES
// case: computed exclusion performs ToPropertyKey once before Get and copy
(function(){
            var log='',key={},source={};
            key[Symbol.toPrimitive]=function(hint){log+='key:'+hint+'|';return 'fixed'};
            source.__defineGetter__('fixed',function(){log+='get-fixed|';return 40});
            source.__defineGetter__('kept',function(){log+='get-kept|';return 2});
            let {[key]:value,...rest}=source;
            return value+rest.kept+'|'+Object.hasOwn(rest,'fixed')+'|'+log;
        })()
// case: nested computed exclusion reuses its canonical property key
(function(){
            var calls=0,key={};
            key[Symbol.toPrimitive]=function(){calls++;return 'x'};
            let {[key]:{value},...rest}={x:{value:40},kept:2};
            return calls+'|'+value+'|'+rest.kept;
        })()
// case: object-rest prescan ignores regexp and template delimiters in computed keys
(function(){
            let {[/[}]/.source]:a,[`${"}"}`]:b,...rest}={"[}]":1,"}":2,kept:3};
            return a+'|'+b+'|'+rest.kept;
        })()
// case: fixed and Symbol exclusions skip a second getter during rest copy
(function(){
            var log='',symbol=Symbol('excluded'),kept=Symbol('kept'),source={};
            source.__defineGetter__('fixed',function(){log+='fixed|';return 1});
            source.__defineGetter__(symbol,function(){log+='excluded-symbol|';return 2});
            source.__defineGetter__('other',function(){log+='other|';return 3});
            source.__defineGetter__(kept,function(){log+='kept-symbol|';return 4});
            const {fixed,[symbol]:symbolic,...rest}=source;
            var symbols=Object.getOwnPropertySymbols(rest);
            return fixed+':'+symbolic+':'+rest.other+':'+rest[kept]+'|'+log+'|'+
                Object.hasOwn(rest,'fixed')+':'+Object.hasOwn(rest,symbol)+':'+
                (symbols.length===1&&symbols[0]===kept);
        })()
// case: ordinary rest copy snapshots enumerable own keys before any Get
(function(){
            var source={},hidden={value:'hidden',writable:true,enumerable:false,configurable:true};
            source.__defineGetter__('a',function(){
                var b={value:'B',writable:true,enumerable:false,configurable:true};
                Object.defineProperty(source,'b',b);
                hidden.enumerable=true;Object.defineProperty(source,'hidden',hidden);
                source.late='late';return 'A';
            });
            source.b='before';Object.defineProperty(source,'hidden',hidden);
            let {...rest}=source;
            return Object.keys(rest).join(',')+'|'+rest.a+':'+rest.b+':'+
                (rest.hidden===undefined)+':'+(rest.late===undefined)+'|'+
                Object.getOwnPropertyDescriptor(source,'b').enumerable+':'+
                Object.getOwnPropertyDescriptor(source,'hidden').enumerable;
        })()
// case: ordinary snapshot uses a live Get after an earlier getter deletes an own key
(function(){
            var proto={later:'prototype'},source=Object.create(proto);
            source.__defineGetter__('first',function(){delete source.later;return 'first'});
            source.later='own';
            const {...rest}=source;
            return Object.keys(rest).join(',')+'|'+rest.first+':'+rest.later+':'+
                source.hasOwnProperty('later');
        })()
// case: copied getters run in integer string then Symbol order after exclusions
(function(){
            var log='',excluded=Symbol('excluded'),kept=Symbol('kept'),source={};
            source.__defineGetter__('10',function(){log+='10|';return 10});
            source.__defineGetter__('2',function(){log+='2|';return 2});
            source.__defineGetter__('a',function(){log+='a|';return 'A'});
            source.__defineGetter__(excluded,function(){log+='excluded|';return 'X'});
            source.__defineGetter__(kept,function(){log+='kept|';return 'K'});
            let {10:ten,[excluded]:symbolic,...rest}=source;
            return ten+':'+symbolic+'|'+Object.getOwnPropertyNames(rest).join(',')+':'+
                rest[2]+rest.a+rest[kept]+'|'+log;
        })()
// case: object conversion precedes computed exclusion and no copy starts on failure
(function(){
            var log='',key={};
            key[Symbol.toPrimitive]=function(){log+='key';return 'x'};
            try{let {[key]:value,...rest}=undefined}catch(error){return error.name+'|'+log}
        })()
// REFERENCE_CASES
// case: sloppy with resolves a missing rest target before copy adds that name
(function(){
            var value='outer',scope={},source={};
            source.__defineGetter__('x',function(){scope.value='late-scope';return 7});
            with(scope){var {...value}=source}
            return value.x+'|'+scope.value+'|'+Object.keys(value).join(',');
        })()
// case: sloppy with retains an existing rest target reference after copy deletes it
(function(){
            var value='outer',scope={value:'early-scope'},source={};
            source.__defineGetter__('x',function(){delete scope.value;return 8});
            with(scope){var {...value}=source}
            return value+'|'+scope.value.x+'|'+Object.keys(scope.value).join(',');
        })()
// ABRUPT_CASES
// case: rest getter throw stops later copied getters and preserves the thrown value
(function(){
            var log='',source={};
            source.__defineGetter__('a',function(){log+='a|';return 1});
            source.__defineGetter__('b',function(){log+='b|';throw 'copy-error'});
            source.__defineGetter__('c',function(){log+='c|';return 3});
            try{let {...rest}=source}catch(error){return log+'caught:'+error}
        })()
// case: rest copy fault closes for-of and keeps the pending fault over close
(function(){
            var log='',done=false,source={};
            source.__defineGetter__('x',function(){log+='G|';throw 'copy-error'});
            var outer={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';if(done)return{done:true};done=true;return{value:source,done:false}},
                return:function(){log+='C|';throw 'close-error'}
            };
            try{for(const {...rest} of outer){}}
            catch(error){return log+'caught:'+error}
        })()
// case: rest target Put fault follows copy then closes the outer iterator
(function(){
            var log='',done=false,scope={},source={x:1};
            Object.defineProperty(scope,'value',{configurable:true,set:function(rest){log+='S:'+rest.x+'|';throw 'put-error'}});
            source.__defineGetter__('y',function(){log+='G|';return 2});
            var outer={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';if(done)return{done:true};done=true;return{value:source,done:false}},
                return:function(){log+='C|';throw 'close-error'}
            };
            try{with(scope){for(var {...value} of outer){}}}
            catch(error){return log+'caught:'+error}
        })()
