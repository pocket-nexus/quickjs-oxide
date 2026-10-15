// DIRECT_CASES
// case: assignment returns its unconsumed RHS while writing flat targets
(function(){
            var identifier=0,holder={fixed:0,computed:0},key='computed';
            var source=[40,1,1],result;
            result=([identifier,holder.fixed,holder[key]]=source);
            return (result===source)+'|'+identifier+'|'+holder.fixed+'|'+holder.computed;
        })()
// case: super fixed and computed properties are valid assignment targets
(function(){
            var proto={
                set first(value){this.left=value},
                set second(value){this.right=value}
            };
            var home={
                __proto__:proto,
                run(values){
                    [super.first,super['second']]=values;
                    return this.left+'|'+this.right;
                }
            };
            return home.run([40,2]);
        })()
// case: defaults use strict undefined and perform NamedEvaluation
(function(){
            var zero,nil,missing,named,arrow,log='';
            [
                zero=(log+='zero|',9),
                nil=(log+='nil|',8),
                missing=(log+='missing|',40),
                named=function(){},
                arrow=()=>{}
            ]=[0,null,undefined];
            return zero+'|'+nil+'|'+missing+'|'+named.name+'|'+arrow.name+'|'+log;
        })()
// case: empty elisions and rest share iterator assignment lowering
(function(){
            var log='',empty={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';return{value:1,done:false}},
                return:function(){log+='C|';return{done:true}}
            };
            []=empty;
            var first,rest;
            [first,,...rest]=[1,2,3,4];
            return log+first+'|'+rest.join(',');
        })()
// case: rest accepts a member target and exhausts the source iterator
(function(){
            var holder={};
            [...holder.values]='ab';
            return Array.isArray(holder.values)+'|'+holder.values.join(',');
        })()
// case: nested array patterns recurse through defaults elisions and rest
(function(){
            var first,second,third,tail,result;
            result=([first,[second=40,,...tail],[[third]]]=[1,[undefined,9,2,3],[[4]]]);
            return (result[0]===1)+'|'+first+'|'+second+'|'+tail.join(',')+'|'+third;
        })()
// REFERENCE_ORDER_CASES
// case: computed expression precedes IteratorNext but ToPropertyKey and Put follow it
(function(){
            var log='',key={},target={},once=false;
            key[Symbol.toPrimitive]=function(hint){log+='K:'+hint+'|';return 'value'};
            Object.defineProperty(target,'value',{set:function(value){log+='P:'+value+'|'}});
            var iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';if(once)return{done:true};once=true;return{value:42,done:false}},
                return:function(){log+='C|';return{done:true}}
            };
            [target[(log+='E|',key)]]=iterator;
            return log;
        })()
// case: with retains an existing property Reference across IteratorNext deletion
(function(){
            var value='outer',scope={value:'scope'},once=false;
            var iterator={
                [Symbol.iterator]:function(){return this},
                next:function(){
                    if(once)return{done:true};once=true;delete scope.value;
                    return{value:40,done:false};
                },
                return:function(){return{done:true}}
            };
            with(scope){[value]=iterator}
            return value+'|'+scope.value;
        })()
// case: with resolves a missing identifier before IteratorNext adds that property
(function(){
            var value='outer',scope={},once=false;
            var iterator={
                [Symbol.iterator]:function(){return this},
                next:function(){
                    if(once)return{done:true};once=true;scope.value='late-scope';
                    return{value:41,done:false};
                },
                return:function(){return{done:true}}
            };
            with(scope){[value]=iterator}
            return value+'|'+scope.value;
        })()
// case: nested target Reference is prepared after the outer step and before the inner step
(function(){
            var value='outer',scope={},outerDone=false,innerDone=false;
            var inner={
                [Symbol.iterator]:function(){return this},
                next:function(){
                    if(innerDone)return{done:true};innerDone=true;delete scope.value;
                    return{value:42,done:false};
                },
                return:function(){return{done:true}}
            };
            var outer={
                [Symbol.iterator]:function(){return this},
                next:function(){
                    if(outerDone)return{done:true};outerDone=true;scope.value='inner-scope';
                    return{value:inner,done:false};
                },
                return:function(){return{done:true}}
            };
            with(scope){[[value]]=outer}
            return value+'|'+scope.value;
        })()
// ITERATOR_CLOSE_CASES
// case: a successful short pattern closes its non-exhausted iterator
(function(){
            var log='',value,iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';return{value:42,done:false}},
                return:function(){log+='C|';return{done:true}}
            };
            [value]=iterator;
            return value+'|'+log;
        })()
// case: a close throw replaces an otherwise successful assignment
(function(){
            var log='',value,iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';return{value:1,done:false}},
                return:function(){log+='C|';throw 'close-error'}
            };
            try{[value]=iterator}catch(error){return log+'caught:'+error}
        })()
// case: a setter fault closes and keeps the pending Put failure over close
(function(){
            var log='',target={},iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';return{value:1,done:false}},
                return:function(){log+='C|';throw 'close-error'}
            };
            Object.defineProperty(target,'value',{set:function(){log+='P|';throw 'put-error'}});
            try{[target.value]=iterator}catch(error){return log+'caught:'+error}
        })()
// case: a computed ToPropertyKey fault closes and keeps its pending failure
(function(){
            var log='',target={},key={},iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';return{value:1,done:false}},
                return:function(){log+='C|';throw 'close-error'}
            };
            key[Symbol.toPrimitive]=function(){log+='K|';throw 'key-error'};
            try{[target[(log+='E|',key)]]=iterator}catch(error){return log+'caught:'+error}
        })()
// case: a default fault closes and keeps its pending failure over close
(function(){
            var log='',value,iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';return{value:undefined,done:false}},
                return:function(){log+='C|';throw 'close-error'}
            };
            try{[value=(log+='D|',function(){throw 'default-error'})()]=iterator}
            catch(error){return log+'caught:'+error}
        })()
// case: IteratorNext failure does not call return
(function(){
            var log='',value,iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';throw 'next-error'},
                return:function(){log+='C|';return{done:true}}
            };
            try{[value]=iterator}catch(error){return log+'caught:'+error}
        })()
// case: rest exhausts naturally without calling return
(function(){
            var log='',index=0,rest,iterator={
                [Symbol.iterator]:function(){log+='I|';return this},
                next:function(){log+='N|';index++;return index<3?{value:index,done:false}:{done:true}},
                return:function(){log+='C|';return{done:true}}
            };
            [...rest]=iterator;
            return rest.join(',')+'|'+log;
        })()
// case: nested close runs inner then outer and preserves the inner failure
(function(){
            var log='',value,outerDone=false,inner={
                [Symbol.iterator]:function(){log+='II|';return this},
                next:function(){log+='IN|';return{value:1,done:false}},
                return:function(){log+='IC|';throw 'inner-close'}
            },outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(outerDone)return{done:true};outerDone=true;return{value:inner,done:false}},
                return:function(){log+='OC|';throw 'outer-close'}
            };
            try{[[value]]=outer}catch(error){return log+'caught:'+error}
        })()
// WRITE_CASES
// case: sloppy readonly Put is ignored while strict readonly Put throws
(function(){
            var loose={};Object.defineProperty(loose,'value',{value:1,writable:false});
            [loose.value]=[2];
            var strictError=(function(){
                'use strict';
                var fixed={};Object.defineProperty(fixed,'value',{value:1,writable:false});
                try{[fixed.value]=[2]}catch(error){return error.name}
            })();
            return loose.value+'|'+strictError;
        })()
// case: sloppy unresolvable Put creates a global while strict Put throws
(function(){
            [__array_assignment_loose__]=[40];
            var loose=globalThis.__array_assignment_loose__;
            delete globalThis.__array_assignment_loose__;
            var strictError=(function(){
                'use strict';
                try{[__array_assignment_strict__]=[2]}catch(error){return error.name}
            })();
            return loose+'|'+strictError+'|'+typeof globalThis.__array_assignment_strict__;
        })()
// case: const and later lexical assignment targets preserve TypeError and TDZ
(function(){
            const fixed=1;var constError,tdzError;
            try{[fixed]=[2]}catch(error){constError=error.name}
            try{[later]=[3];let later}catch(error){tdzError=error.name}
            return fixed+'|'+constError+'|'+tdzError;
        })()
// LOOP_CASES
// case: for-of accepts a leading Array literal fixed member target
(function(){
            var receiver,value;
            Object.defineProperty(Array.prototype,'x',{
                configurable:true,
                set:function(next){receiver=this;value=next}
            });
            try{for([].x of [42]){}}
            finally{delete Array.prototype.x}
            return Array.isArray(receiver)+'|'+receiver.length+'|'+value;
        })()
// case: for-of accepts a leading Object literal fixed member target
(function(){
            var receiver,value;
            Object.defineProperty(Object.prototype,'x',{
                configurable:true,
                set:function(next){receiver=this;value=next}
            });
            try{for({}.x of [42]){}}
            finally{delete Object.prototype.x}
            return (Object.getPrototypeOf(receiver)===Object.prototype)+'|'+
                Object.keys(receiver).length+'|'+value;
        })()
// case: for-of accepts a leading Array literal computed member target
(function(){
            var calls=0,key={},receivers=[],values=[];
            key[Symbol.toPrimitive]=function(hint){calls++;return hint==='string'?'x':'wrong'};
            Object.defineProperty(Array.prototype,'x',{
                configurable:true,
                set:function(next){receivers.push(this);values.push(next)}
            });
            try{for([][key] of [40,2]){}}
            finally{delete Array.prototype.x}
            return calls+'|'+values.join(',')+'|'+(receivers[0]!==receivers[1])+'|'+
                Array.isArray(receivers[0])+':'+Array.isArray(receivers[1]);
        })()
// case: for-of accepts a leading Object literal computed member target
(function(){
            var calls=0,key={},receiver,value;
            key[Symbol.toPrimitive]=function(hint){calls++;return hint==='string'?'x':'wrong'};
            Object.defineProperty(Object.prototype,'x',{
                configurable:true,
                set:function(next){receiver=this;value=next}
            });
            try{for({}[key] of [41]){}}
            finally{delete Object.prototype.x}
            return calls+'|'+(Object.getPrototypeOf(receiver)===Object.prototype)+'|'+value;
        })()
// case: for-in accepts a leading Array literal member target
(function(){
            var receivers=[],values=[];
            Object.defineProperty(Array.prototype,'x',{
                configurable:true,
                set:function(next){receivers.push(this);values.push(next)}
            });
            try{for([].x in {a:1,b:2}){}}
            finally{delete Array.prototype.x}
            return values.join(',')+'|'+(receivers[0]!==receivers[1])+'|'+
                Array.isArray(receivers[0])+':'+Array.isArray(receivers[1]);
        })()
// case: for-in accepts a leading Object literal member target
(function(){
            var receivers=[],values=[];
            Object.defineProperty(Object.prototype,'x',{
                configurable:true,
                set:function(next){receivers.push(this);values.push(next)}
            });
            try{for({}.x in {a:1,b:2}){}}
            finally{delete Object.prototype.x}
            return values.join(',')+'|'+(receivers[0]!==receivers[1])+'|'+
                (Object.getPrototypeOf(receivers[0])===Object.prototype)+':'+
                (Object.getPrototypeOf(receivers[1])===Object.prototype);
        })()
// case: for-of array assignment heads update existing identifiers
(function(){
            var left,right,log='';
            for([left,right] of [[1,2],[3,4]])log+=left+':'+right+'|';
            return log+'last:'+left+':'+right;
        })()
// case: for-in array assignment heads destructure each yielded string key
(function(){
            var first,second,log='';
            for([first,second] in {ab:1,cd:2})log+=first+second+'|';
            return log+'last:'+first+second;
        })()
// case: for-of array assignment heads support member targets and rest
(function(){
            var target={};
            for([target.head,...target.tail] of [[1,2,3],[40,2]]){}
            return target.head+'|'+target.tail.join(',');
        })()
// case: assignment failure closes inner then outer and preserves the pending fault
(function(){
            var log='',target={},outerDone=false,inner={
                [Symbol.iterator]:function(){log+='II|';return this},
                next:function(){log+='IN|';return{value:1,done:false}},
                return:function(){log+='IC|';throw 'inner-close'}
            },outer={
                [Symbol.iterator]:function(){log+='OI|';return this},
                next:function(){log+='ON|';if(outerDone)return{done:true};outerDone=true;return{value:inner,done:false}},
                return:function(){log+='OC|';throw 'outer-close'}
            };
            Object.defineProperty(target,'value',{set:function(){log+='P|';throw 'put-error'}});
            try{for([target.value] of outer){}}
            catch(error){return log+'caught:'+error}
        })()
