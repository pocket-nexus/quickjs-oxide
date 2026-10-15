// CASES
// case: mixed defaults and BindingPatterns execute across callable surfaces
(function(){
            var out=[];
            function ordinary([a]=[40],b=a+2){return b}
            out.push(ordinary());
            out.push((([a]=[40],b=a+2)=>b)());
            out.push(({method([a]=[40],b=a+2){return b}}).method());
            out.push(Function('[a]=[40]','b=a+2','return b')());
            return out.join('|');
        })()
// case: recursive leaf and whole-pattern defaults initialize in source order
(function(){
            var calls=0;
            return (function({a:[b=40]}={a:[]},c=(calls++,b+2)){
                return b+'|'+c+'|'+calls;
            })();
        })()
// case: leaf defaults compose with a later identifier rest across callable surfaces
(function(){
            function ordinary([a=1],...rest){return a+'|'+rest.join(',')}
            var arrow=([a=1],...rest)=>a+'|'+rest.join(',');
            var object={method([a=1],...rest){return a+'|'+rest.join(',')}};
            return ordinary([],2,3)+';'+arrow([],2,3)+';'+object.method([],2,3);
        })()
// case: whole-pattern defaults compose with a later identifier rest across callable surfaces
(function(){
            function ordinary([a]=[1],...rest){return a+'|'+rest.join(',')}
            var arrow=([a]=[1],...rest)=>a+'|'+rest.join(',');
            var object={method([a]=[1],...rest){return a+'|'+rest.join(',')}};
            return ordinary(undefined,2,3)+';'+arrow(undefined,2,3)+';'+
                object.method(undefined,2,3);
        })()
// case: an assignment anywhere in the formal list selects the parameter-environment rest entry
(function(){
            function later([a],b=1,...rest){return a+'|'+b+'|'+rest.join(',')}
            function earlier(a=0,[b],...rest){return a+'|'+b+'|'+rest.join(',')}
            function empty({},b=1,...rest){return b+'|'+rest.join(',')}
            return later([40],undefined,2,3)+';'+earlier(undefined,[40],2,3)+';'+
                empty({},undefined,2,3);
        })()
// case: a whole-pattern default resolves outside body var hoists
(function(){
            var outer=42;
            return (function([a]=[outer],b=1){
                var outer=0;
                return a;
            })();
        })()
// case: a later nested default pre-scans the whole formal list
(function(){
            var key='outer';
            return (function({[String(key)]:value},[x=1]){
                var key='body';
                return value;
            })({outer:42,undefined:1},[]);
        })()
// case: a pattern leaf cannot read a later parameter in its TDZ
(function(){
            try { return (function([a=b],b=1){return a})([]) }
            catch(error) { return error.name }
        })()
// case: a computed key cannot read a later parameter in its TDZ
(function(){
            try { return (function({[b]:a},b=1){return a})({undefined:42}) }
            catch(error) { return error.name }
        })()
// case: pattern BoundNames copy into the body after every initializer
(function([a],b=(a=42)){return a})([1])
// case: an initializer closure retains the parameter cell after the body copy
(function(){
            var read;
            return (function([a],b=(read=()=>a,0)){
                a=2;
                return a+'|'+read();
            })([1]);
        })()
// case: a leaf-default closure also retains the parameter cell
(function(){
            var read;
            return (function([a=(read=()=>a,1)]){
                a=2;
                return a+'|'+read();
            })([]);
        })()
// case: parameter expressions keep sloppy arguments unmapped
(function([a],b=0){
            arguments[0]=[9];
            a=7;
            return a+'|'+arguments[0][0];
        })([1])
// case: a zero-BoundName pattern still creates a parameter environment
(function(){
            var source={};
            return (function({}=source){
                var source=null;
                return 42;
            })();
        })()
// case: function length distinguishes leaf defaults from whole-pattern defaults
[
            (function([a=1],b){}).length,
            (function([a]=[1],b){}).length,
            (function(a,[b=1],c){}).length,
            (function(a,[b]=[1],c){}).length,
            (function(a,...[b=1]){}).length,
            (function(a=1,...[b]){}).length
        ].join('|')
// case: QuickJS accepts an unreachable default on a rest BindingPattern
(function(){
            var f=function(...[a]=[99]){
                return String(a)+'|'+arguments.length;
            };
            return f.length+';'+f()+';'+f(42);
        })()
// case: setter defaults and getter rest BindingPatterns retain QuickJS arity quirks
(function(){
            var value;
            var object={
                set item([a]=[1]){value=a},
                get read(...[a]){return a}
            };
            object.item=[42];
            var getter=Object.getOwnPropertyDescriptor(object,'read').get;
            return value+'|'+String(object.read)+'|'+getter.length;
        })()
// ERROR_CASES
// case: a pattern BoundName cannot duplicate a defaulted identifier parameter
function invalid([value],value=1){}
// case: a defaulted leaf cannot duplicate a BoundName in the same pattern
function invalid([value,value=1]){}
// case: setter patterns apply duplicate-BoundName early errors
({set item([value,value=1]){}})
