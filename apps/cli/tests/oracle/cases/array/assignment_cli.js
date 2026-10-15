// mode: print
// STACK_CASES
// case: direct IteratorNext fault points at its identifier target
(function outer(){var x,it={[Symbol.iterator]:function(){return this},next:function nextFault(){throw new Error('next')}};[x]=it})()
// case: computed target IteratorNext fault points at its member site
(function outer(){var obj={},key='x',it={[Symbol.iterator]:function(){return this},next:function nextFault(){throw new Error('next')}};[obj[key]]=it})()
// case: second-step rest fault points at the rest identifier
(function outer(){var x,rest,it={i:0,[Symbol.iterator]:function(){return this},next:function restFault(){if(this.i++)throw new Error('next');return{value:1,done:false}}};[x,...rest]=it})()
// case: later elision fault retains the preceding identifier site
(function outer(){var x,it={i:0,[Symbol.iterator]:function(){return this},next:function elisionFault(){if(this.i++)throw new Error('next');return{value:1,done:false}}};[x,,]=it})()
// case: nested rest drain fault retains the preceding outer target site
(function outer(){var x,y,it={i:0,[Symbol.iterator]:function(){return this},next:function nestedRestFault(){if(this.i++)throw new Error('next');return{value:1,done:false}}};[x,...[y]]=it})()
// case: later nested outer-step fault retains the preceding target site
(function outer(){var a,x,it={i:0,[Symbol.iterator]:function(){return this},next:function laterNestedFault(){if(this.i++)throw new Error('next');return{value:1,done:false}}};[a,[x]]=it})()
// case: leading nested outer-step fault points at the nested opener
(function outer(){var x,it={[Symbol.iterator]:function(){return this},next:function leadingNestedFault(){throw new Error('next')}};[[x]]=it})()
// case: post-elision nested outer-step fault retains the pattern opener
(function outer(){var x,it={i:0,[Symbol.iterator]:function(){return this},next:function elidedNestedFault(){if(this.i++)throw new Error('next');return{value:1,done:false}}};[,[x]]=it})()
// case: nested iterator acquisition fault retains the preceding outer target site
(function outer(){var a,x,inner={[Symbol.iterator]:function innerAcquireFault(){throw new Error('acquire')}};[a,[x]]=[1,inner]})()
// case: nested IteratorNext fault points at the inner identifier
(function outer(){var x,inner={[Symbol.iterator]:function(){return this},next:function innerFault(){throw new Error('inner')}};[[x]]=[inner]})()
// case: normal IteratorClose fault keeps the final identifier target
(function outer(){var x,it={[Symbol.iterator]:function(){return this},next:function(){return{value:1,done:false}},return:function closeFault(){throw new Error('close')}};[x]=it})()
// PARSER_CASES
// case: array assignment rest cannot have a trailing comma
var value;[...value,]=[]
// case: array assignment rest must be the final element
var first,last;[...first,last]=[]
// case: array assignment rest cannot carry an initializer
var rest;[...rest=[]]=[]
// case: array assignment rest requires a target
var a;[...]=[];
// case: nested array rest default wins over its invalid leaf
var a;[...[true&&a]=[]]=[];
// case: nested object rest default wins over its invalid leaf
var a;[...{p:true&&a}=[]]=[];
// case: validated nested array rest default wins over its invalid leaf
var a;({p:[...[true&&a]=[]]}={});
// case: validated nested object rest default wins over its invalid leaf
var a;({p:[...{q:true&&a}=[]]}={});
// case: later invalid leaf retains its diagnostic after a direct object pattern
var a,x;[{x},true&&a]=[];
// case: later invalid leaf retains its diagnostic after a recursive object pattern
var a,x;[[{x}],true&&a]=[];
// case: later invalid for-of leaf retains its diagnostic after an object pattern
var a,x;for([{x},true&&a] of []){}
// case: rest-last error wins after a nested object pattern
var a,x;[...{x},true&&a]=[];
// case: invalid logical target reports at the following delimiter
var a;[true&&a]=[];
// case: invalid call target reports at the following delimiter
var a;[a()]=[];
// case: invalid new target reports at the following delimiter
var a;[new a]=[];
// case: array assignment pattern cannot be a compound target
var value;[value]+=[1]
// case: array assignment pattern cannot be a logical-and target
var value;[value]&&=[1]
// case: array assignment pattern cannot be a logical-or target
var value;[value]||=[1]
// case: array assignment pattern cannot be a nullish target
var value;[value]??=[1]
// case: strict array assignment rejects eval
'use strict';var value;[eval]=[value]
// case: invalid object assignment leaf retains its target diagnostic
var a;({p: true && a} = {});
// case: later invalid object leaf wins after an earlier nested object pattern
var a,x;({p:[{x}],q:true&&a}={});
// case: strict object assignment shorthand preserves the QuickJS target diagnostic
'use strict';var x;({eval}={});
