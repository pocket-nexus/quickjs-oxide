// CASES
// case: BindingPatterns execute across synchronous callable surfaces
(function(){
            var out=[];
            function declaration([a],{b}){return a+b}
            out.push(declaration([40],{b:2}));
            out.push((function([a]){return a+2})([40]));
            out.push((([a])=>a+2)([40]));
            out.push(({base:40,method([a]){return this.base+a}}).method([2]));
            out.push(Function('[a]','return a+2')([40]));
            var assigned;
            var setter={set value([a]){assigned=a}};
            setter.value=[42];
            out.push(assigned);
            return out.join('|');
        })()
// case: recursive array and object patterns retain internal rest semantics
(function([a,,[b,...tail]],{x:y,[String('z')]:z,...rest}){
            return a+'|'+b+'|'+tail.join(',')+'|'+y+'|'+z+'|'+rest.extra;
        })([1,0,[2,3,4]],{x:5,z:6,extra:7})
// case: a pattern parameter makes sloppy arguments unmapped
(function([a]){
            arguments[0]=[9];
            a=7;
            return a+'|'+arguments[0][0];
        })([1])
// case: rest BindingPatterns preserve QuickJS physical-slot and length quirks
(function(...[a,b]){
            return a+b+'|'+arguments.length+'|'+(function(...[x]){}).length+'|'+
                (function(x,...[y]){}).length;
        })(40,2)
// case: empty rest patterns retain QuickJS bytecode length publication quirks
[
            (function(...[]){}).length,
            (function(...{}){}).length,
            (function(...[[]]){}).length,
            (function(...[,]){}).length,
            (function(...[a]){}).length,
            (function(...{a}){}).length,
            (function(...[]){var local}).length,
            (function(...[]){return arguments}).length,
            (function(...[]){return this}).length,
            (function(...[]){return new.target}).length,
            (function(...[]){return function(){return this}}).length,
            (function(...[]){return ()=>this}).length,
            (function named(...[]){return named}).length,
            (function named(...[]){}).length,
            (function(first,...[]){}).length
        ].join('|')
// case: an identifier rest may follow an ordinary BindingPattern
(function mixed([a],...rest){
            return a+rest[0]+'|'+mixed.length+'|'+arguments.length;
        })([40],2)
// case: body var and function hoists occur after parameter destructuring
(function(){
            var retained=(function([a]){var a;return a})([42]);
            var replaced=(function([a]){
                function a(){return 42}
                return typeof a+'|'+a();
            })([1]);
            return retained+';'+replaced;
        })()
// case: computed keys see root vars before body initialization
(function(){
            var key='outer';
            var root=(function({[String(key)]:value}){
                var key='body';
                return value;
            })({undefined:42,outer:1});
            var lexical='outer';
            var body=(function({[lexical]:value}){
                let lexical='body';
                return value;
            })({outer:42});
            return root+'|'+body;
        })()
// case: computed-key closures retain the shared function-root binding
(function(){
            var saved;
            function capture(fn){saved=fn;return 'undefined'}
            return (function({[capture(()=>key)]:value}){
                var key='body';
                return value+'|'+saved();
            })({undefined:42});
        })()
// case: direct eval observes initialized pattern BoundNames
(function([a]){return eval('a')})([42])
// case: direct eval in a computed key runs before the body scope
(function({[eval('"key"')]:value}){
            return value;
        })({key:42})
// case: a pattern BoundName overwrites arguments only after computed keys
(function({[arguments]:arguments}){
            return arguments;
        })({undefined:1,"[object Arguments]":42})
// case: nested-arrow eval retains the missing parent body segment
(function({[(()=>eval("typeof key"))()]:value}){
            var key="body";
            return value;
        })({undefined:42})
// case: an arrow pattern preserves lexical arguments
(function(){return (([a])=>a+arguments[1])([40])})(0,2)
// case: compound assignment does not set QuickJS SKIP_HAS_ASSIGNMENT
(function(){
            var key=0;
            return (function({[(key+=1)]:value}){return value})({1:42});
        })()
// case: parameter array destructuring performs iterator close
(function(){
            var closed=0;
            var iterable={
                [Symbol.iterator](){
                    var next=0;
                    return {
                        next(){next++;return next===1?{value:42,done:false}:{done:true}},
                        return(){closed++;return {done:true}}
                    };
                }
            };
            return (function([a]){return a})(iterable)+'|'+closed;
        })()
// case: ordinary methods preserve HomeObject with pattern parameters
(function(){
            var base={value(){return 40}};
            var object={__proto__:base,value([a]){return super.value()+a}};
            return object.value([2]);
        })()
// ERROR_CASES
// case: a BoundName cannot duplicate another formal parameter
function invalid([value],value){}
// case: duplicate BoundNames inside a pattern are rejected
function invalid([value,value]){}
// case: a strict directive is forbidden with non-simple formals
function invalid([value]){'use strict'}
// case: a rest BindingPattern rejects a trailing comma
(...[value],)=>0
