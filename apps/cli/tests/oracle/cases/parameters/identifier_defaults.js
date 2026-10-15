// case: ordinary defaults distinguish undefined from supplied values
(function(){
            function f(a=40,b=a+2){return a+'|'+b}
            return f(undefined)+';'+f(41,1);
        })()
// case: a supplied non-undefined value skips its initializer
(function(){
            var calls=0;
            function f(a=(calls++,42)){return a}
            var value=f(7);
            return calls+'|'+value;
        })()
// case: arrow defaults resolve prior parameter bindings
((a=40,b=a+2)=>b)()
// case: object method defaults retain dynamic this
({base:40,method(a=this.base+2){return a}}).method()
// case: Function constructor reuses ordinary default parsing
Function('a=40','b=a+2','return b')()
// case: a later parameter remains in TDZ during an earlier initializer
(function(){
            try {(function(a=b,b=1){})()}
            catch(error) {return error.name}
            return 'none';
        })()
// case: a parameter remains in its own TDZ during self initialization
(function(){
            try {(function(a=a){})()}
            catch(error) {return error.name}
            return 'none';
        })()
// case: default parameters make sloppy arguments unmapped
(function(a=1){
            arguments[0]=9;
            var before=a;
            a=7;
            return before+'|'+a+'|'+arguments[0];
        })(2)
// case: function length stops before the first default
(function(){
            var ordinary=function(a,b=1,c){};
            var arrow=(a=1,b)=>0;
            var method=({method(a,b=1){}}).method;
            var dynamic=Function('a','b=1','c','');
            return ordinary.length+'|'+arrow.length+'|'+method.length+'|'+dynamic.length;
        })()
// case: body hoists replace the raw argument but not the initializer closure cell
(function(){
            var read;
            function f(a=1,b=(read=()=>a,0)){
                function a(){return 42}
                return typeof a+'|'+a()+'|'+read();
            }
            return f();
        })()
// case: anonymous function defaults receive the parameter name
(function(a=function(){}){return a.name})()
// case: anonymous arrow defaults receive the parameter name
((a=()=>0)=>a.name)()
// case: private function names stay outside same-named body bindings
(function(){
            var read=(function f(a=f){var f;return typeof a+'|'+(a===undefined)+'|'+typeof f})();
            var closure=(function f(a=()=>f){var f=1;return typeof a()+'|'+(a()===f)})();
            var write=(function f(a=(f=1)){var f;return typeof f+'|'+(f===1)})();
            var strict;
            try {
                (function(){'use strict';return (function f(a=(f=1)){var f})()})()
            } catch(error) { strict=error.name+'|'+error.message }
            return read+';'+closure+';'+write+';'+strict;
        })()
// case: an identifier default composes with an identifier rest parameter
(function(a=40,...rest){return a+rest[0]+rest.length})(undefined,1)
// case: QuickJS separates the body raw argument from the initializer closure cell
(function(){
            var read;
            function f(a=1,b=(read=()=>a,0)){
                a=2;
                return a+'|'+read();
            }
            return f();
        })()
