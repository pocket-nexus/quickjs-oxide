// case: parameter target / a parameter eval var is visible to later parameters and authored body lookup
(function(a=eval("var x=1"),b=x,c=()=>x){return [b,x,c()].join("|")})()
// case: parameter target / a parameter eval function declaration is hoisted into the argument variable object
(function(a=eval("var r=x();function x(){return 42};r"),b=x()){return [a,b,x()].join("|")})()
// case: parameter target / a sloppy eval block function receives its Annex B argument-variable alias
(function(a=eval("{function x(){return 42}}"),b=typeof x,c=x()){return [b,c,x()].join("|")})()
// case: parameter target / an eval var cannot redeclare an earlier parameter lexical cell
(function(){var touch=0;try{(function(a=eval("touch=1;var a")){})()}catch(e){return e.name+"|"+touch}return "none"})()
// case: parameter target / an eval var cannot redeclare a later parameter lexical cell
(function(){var touch=0;try{(function(a=eval("touch=1;var b"),b=1){})()}catch(e){return e.name+"|"+touch}return "none"})()
// case: parameter target / an eval function cannot redeclare a later parameter lexical cell
(function(){var touch=0;try{(function(a=eval("touch=1;function b(){}"),b=1){})()}catch(e){return e.name+"|"+touch}return "none"})()
// case: parameter target / an eval assignment mutates the parameter cell while the body keeps its copied argument
(function(a=1,b=eval("a=7"),c=a,d=()=>a){return [c,d(),a].join("|")})()
// case: parameter target / a default anywhere moves a preceding plain formal into the parameter lexical environment
(function(a,b=eval("a=7"),c=a,d=()=>a){return [c,d(),a].join("|")})(1)
// case: parameter target / a BindingPattern copies its final parameter cell value into the body environment
(function({x:a=1}={},b=eval("a=7"),c=a,d=()=>a){return [c,d(),a].join("|")})()
// case: parameter target / an eval assignment to a later parameter observes its temporal dead zone
(function(){try{return (function(a=eval("b=7"),b=1){return "none"})()}catch(e){return e.name}})()
// case: parameter target / a closure created by parameter eval captures the parameter cell rather than the body copy
(function(a=1,b=eval("var read=()=>a")){a=2;return [read(),a].join("|")})()
// case: parameter target / eval dynamic lookup precedes the private function name but authored lookup keeps the private name
(function(){return (function f(a=eval("var f=7"),b=eval("f"),c=()=>f){return [b,typeof c(),typeof f].join("|")})()})()
// case: parameter target / an eval lexical declaration remains local to the eval invocation
(function(a=eval("let x=1;x"),b=typeof x){return [a,b,typeof x].join("|")})()
// case: environment split / body eval creates a body variable that shadows the argument eval variable
(function(a=eval("var x=1"),b=()=>x){eval("var x=2");return [x,b()].join("|")})()
// case: environment split / an authored body var shadows an argument eval variable
(function(a=eval("var x=1"),b=()=>x){var x=2;return [x,b()].join("|")})()
// case: environment split / an authored body function shadows an argument eval variable
(function(a=eval("var x=1"),b=()=>x){function x(){return 2}return [x(),b()].join("|")})()
// case: environment split / nested parameter eval reuses the argument variable object
(function(a=eval("var x=1;eval('x=2;var y=3')"),b=()=>x+y){return [x,y,b()].join("|")})()
// case: environment split / nested body eval targets the body variable object before the argument variable object
(function(a=eval("var x=1"),b=()=>x){eval("eval('var x=2')");return [x,b()].join("|")})()
// case: environment split / body eval updates exact body bindings without overwriting the argument variable object
(function(a=1,b=eval("var x=9"),c=()=>x){eval("a=2;var x=3");return [a,x,c()].join("|")})()
// case: environment split / a function created by body eval captures the body binding
(function(a=eval("var x=9"),b=()=>x){var x=2;eval("var f=()=>x");x=3;return [f(),b()].join("|")})()
// case: dynamic object / deleting an argument eval var removes authored and later-parameter visibility
(function(a=eval("var x=1;delete x"),b=typeof x){return [a,b,typeof x].join("|")})()
// case: dynamic object / an eval function closure survives deletion of its dynamic binding
(function(){var saved;return (function(a=eval("function x(){return 42};saved=x;delete x"),b=typeof x){return [b,saved()].join("|")})()})()
// case: dynamic object / a repeated object-backed var declaration resets the property to undefined
(function(a=eval("var x=1"),before=x,reset=eval("var x"),after=typeof x){return [before,typeof reset,after,typeof x].join("|")})()
// case: dynamic object / closures relay the argument variable object after its creating frame exits
(function(){var pair=(function(a=eval("var x=1"),get=()=>x,set=v=>x=v){return [get,set]})();var before=pair[0]();pair[1](2);return [before,pair[0]()].join("|")})()
// case: arguments / the parameter and body share one unmapped arguments object
(function(a=1,b=(eval("0"),arguments[0]=9,a),c=arguments[0]){return [a,b,c,arguments[0]].join("|")})(undefined)
// case: arguments / a BindingPattern initializer reads the synthetic parameter arguments cell
(function({x=arguments[0].answer}={},a=eval("0")){return x})({x:undefined,answer:42})
// case: arguments / a BindingPattern initializer closure captures the synthetic parameter arguments cell
(function({x=()=>arguments[0].answer}={},a=eval("0")){return x()})({x:undefined,answer:42})
// case: arguments / an eval var cannot redeclare the synthetic parameter arguments cell
(function(){try{(function(a=eval("var arguments")){})()}catch(e){return e.name}return "none"})()
// case: arguments / an arrow parameter eval may create arguments in the argument variable object
(function(){return ((a=eval("var arguments=7"),b=arguments)=>[b,arguments].join("|"))()})()
// case: arguments / inherited strict parameter eval does not receive QuickJS's implicit arguments binding
(function(){"use strict";return (function(a=eval("typeof arguments"),b=eval("try{arguments;'none'}catch(e){e.name}")){return [a,b].join("|")})()})()
// case: arguments / a named arguments parameter remains in parameter scope while body lookup sees the implicit object
(function(arguments=1,a=eval("0"),b=arguments){return [b,typeof arguments,arguments===b].join("|")})()
// case: arguments / an eval var cannot redeclare a named arguments parameter
(function(){try{(function(arguments=1,a=eval("var arguments")){})()}catch(e){return e.name}return "none"})()
// case: arguments / an authored body var named arguments precedes the body eval variable object
(function(a=eval("0")){var arguments;eval("var arguments=7");return [typeof arguments,String(arguments)].join("|")})()
// case: arguments / an authored body function named arguments precedes the body eval variable object
(function(a=eval("0")){function arguments(){return 3}eval("var arguments=7");return [typeof arguments,String(arguments)].join("|")})()
// case: arguments / a descendant arrow eval does not synthesize arguments in the outer parameter environment
(function(a=(()=>eval("typeof arguments"))()){return a})()
// case: arguments / an authored arguments capture is appended to the caller closure suffix seen by eval
(function(a=(()=>[eval("typeof arguments"),arguments.length].join("|"))()){return a})(undefined)
// case: arguments / a descendant arrow arguments capture is appended to the caller closure suffix seen by eval
(function(a=(()=>{var g=()=>arguments;return eval("typeof arguments")})()){return a})(undefined)
// case: entry ordering / a script pseudo-binding prologue precedes an earlier global function hoist
function f(){};var a=(p=eval("0"))=>p;a()
// case: scope switch / a computed parameter key without any default remains in the function variable scope
(function({[eval("var x=7")]:a}){var x;return [typeof x,x].join("|")})({7:0})
// case: scope switch / any default switches a computed parameter eval to the separate argument variable object
(function({[eval("var x=7")]:a},b=0,c=x){var x;return [c,typeof x].join("|")})({7:0})
// case: strict eval / a strict eval source mutates imported parameters but does not leak its var declaration
(function(a=1,b=eval("'use strict';a=2;var x=3;typeof x"),c=a){return [b,c,a,typeof x].join("|")})()
// case: strict eval / direct eval inherits strictness from the containing function bytecode
(function(){"use strict";return (function(a=1,b=eval("a=2;var x=3;typeof x"),c=a){return [b,c,a,typeof x].join("|")})()})()
