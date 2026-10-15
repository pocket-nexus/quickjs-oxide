// VALUE_CASES
// case: custom iterator protocol
(function(){function R(v,d){this.value=v;this.done=d};function I(){this.i=0};I.prototype.next=function(){this.i++;return new R(this.i,this.i>2)};function X(){};X.prototype[Symbol.iterator]=function(){return new I};var s='';for(var v of new X)s+=v;return s})()
// case: iterator method receiver and next order
(function(){function R(v,d){this.value=v;this.done=d};function I(x){this.x=x;this.i=0};I.prototype.next=function(){this.x.log+='n';this.i++;return new R(this.i,this.i>1)};function X(){this.log=''};X.prototype[Symbol.iterator]=function(){this.log+='i';return new I(this)};var x=new X,s='';for(var v of x)s+=v;return s+'|'+x.log})()
// case: string iterator advances by Unicode code point
(function(){var s='';for(var c of 'A\uD83D\uDCA9\uD800B\uDC00')s+=c.length+':'+c.charCodeAt(0)+'|';return s})()
// case: string iterator exposes the shared iterator prototype contract
(function(){var i='\uD83D\uDCA9'[Symbol.iterator](),a=i.next(),b=i.next();return (i[Symbol.iterator]()===i)+'|'+a.done+'|'+a.value.length+'|'+b.done+'|'+b.value})()
// case: fixed member assignment target
(function(){function B(){this.value=''}var b=new B,s='';for(b.value of 'xy')s+=b.value;return s+'|'+b.value})()
// case: computed member assignment target
(function(){function B(){this.value=''}var b=new B,k='value',s='';for(b[k] of 'xy')s+=b[k];return s+'|'+b.value})()
// case: var binding keeps the final iterated value
(function(){var s='';for(var value of 'ab')s+=value;return s+'|'+value})()
// case: existing identifier assignment target
(function(){var value='',s='';for(value of 'ab')s+=value;return s+'|'+value})()
// case: async-named fixed and computed member assignment targets
(function(){var async={x:0,k:0,of:0};for(async.x of [1]){}for(async['k'] of [2]){}for(async.of of [3]){}return async.x+'|'+async.k+'|'+async.of})()
// case: async ambiguity probe leaves Annex B HTML comments to the real lexer
(function(){var async=0;for(async <!-- comment
 of [1]){}for(async
 --> comment
 of [2]){}return async})()
// case: let binding receives a fresh captured cell
(function(){var f,g,i=0;for(let value of 'ab'){i++;if(i===1)f=function(){return value};else g=function(){return value}}return f()+'|'+g()+'|'+(f()!==g())})()
// case: lexical head binding is in its temporal dead zone for the right operand
(function(){let value='ab';try{for(let value of value)value}catch(e){return e.name+'|'+e.message}})()
// case: const simple binding
(function(){var s='';for(const value of 'ab')s+=value;return s})()
// case: const array binding fills exhausted elements with undefined
(function(){var s='';for(const [a,b] of [[1,2],[3]])s+=a+':'+b+'|';return s})()
// case: let array bindings receive fresh captured cells
(function(){var f,g,i=0;for(let [a,b] of [[1,2],[3,4]]){i++;if(i===1)f=function(){return a+':'+b};else g=function(){return a+':'+b}}return f()+'|'+g()})()
// case: var array binding accepts holes and a trailing comma
(function(){var s='';for(var [a,,b,] of [[1,2,3],[4,5,6]])s+=a+':'+b+'|';return s+'last:'+a+':'+b})()
// case: for-in declaration array binding iterates the yielded string key
(function(){var s='';for(const [a,b,] in {ab:1,cd:2})s+=a+b;return s})()
// case: for-of object bindings cover var let const and fixed computed recursive properties
(function(){var result='';for(var {fixed} of [{fixed:'v'}])result+=fixed;for(let {['computed']:computed} of [{computed:'l'}])result+=computed;for(const {nested:{value}} of [{nested:{value:'c'}}])result+=value;return result})()
// case: for-in object bindings cover var let const and fixed computed recursive properties
(function(){var result='';for(var {0:first} in {ab:1})result+=first;for(let {[1]:second} in {ab:1})result+=second;for(const {constructor:{name}} in {ab:1})result+=name;return result})()
// case: for-of object rest excludes consumed keys
(function(){var result='';for(const {skip,...rest} of [{skip:0,a:1},{skip:0,b:2}])result+=rest.a||rest.b;return result})()
// case: for-in object rest copies the remaining boxed string index
(function(){var result='';for(const {0:first,...rest} in {ab:1,cd:2})result+=first+rest[1];return result})()
// case: for-of recursively nested array bindings receive fresh lexical cells
(function(){var first,second,i=0;for(const [[value]=[0]] of [[[40]],[]]){if(i++===0)first=function(){return value};else second=function(){return value}}return first()+'|'+second()})()
// case: nested for-of values
(function(){var s='';for(var a of 'ab')for(var b of '12')s+=a+b;return s})()
// case: loop Script completion tracks the last entered body
var forOfCompletion='';for(var forOfValue of 'ab'){forOfCompletion+=forOfValue;forOfCompletion}
// case: zero-iteration Script completion remains undefined
for(var x of ''){9}
// CLOSE_CASES
// case: natural exhaustion does not call return
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.i++;return new R(this.i,this.i>2)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,s='';for(var v of new X(l))s+=v;return s+'|'+l.s})()
// case: break closes the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;for(var v of new X(l))break;return v+'|'+l.s})()
// case: same-loop continue does not close the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.i++;return new R(this.i,this.i>2)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,s='';for(var v of new X(l)){s+=v;continue}return s+'|'+l.s})()
// case: same-loop labelled continue does not close the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.i++;return new R(this.i,this.i>2)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,s='';loop:for(var v of new X(l)){s+=v;continue loop}return s+'|'+l.s})()
// case: throw caught inside the loop does not close the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.i++;return new R(this.i,this.i>2)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,s='';for(var v of new X(l)){try{throw v}catch(e){s+=e;continue}}return s+'|'+l.s})()
// case: return expression is evaluated before iterator close
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(4,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;var value=(function(){for(var v of new X(l))return (l.s+='e',v)})();return value+'|'+l.s})()
// case: body throw closes but close throw cannot replace the pending throw
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';throw 9};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{for(var v of new X(l))throw 7}catch(e){return e+'|'+l.s}})()
// case: body throw suppresses a primitive iterator-close result
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return 1};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{for(var v of new X(l))throw 7}catch(e){return e+'|'+l.s}})()
// case: non-callable return replaces break but not a pending throw
(function(){function R(v,d){this.value=v;this.done=d};function I(){this.return=1};I.prototype.next=function(){return new R(1,false)};function X(){};X.prototype[Symbol.iterator]=function(){return new I};var normal,pending;try{for(var a of new X)break}catch(e){normal=e.name};try{for(var b of new X)throw 7}catch(e){pending=e};return normal+'|'+pending})()
// case: break is replaced by a close throw
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';throw 9};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{for(var v of new X(l))break}catch(e){return e+'|'+l.s}})()
// case: return is replaced by a close throw
(function(){function R(v,d){this.value=v;this.done=d};function I(){};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){throw 9};function X(){};X.prototype[Symbol.iterator]=function(){return new I};try{return (function(){for(var v of new X)return 5})()}catch(e){return e}})()
// case: next throw does not call return
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){throw 8};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{for(var v of new X(l))v}catch(e){return e+'|'+l.s}})()
// case: array binding closes a non-exhausted inner iterator before the body
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.l.s+='n';return new R(++this.i,false)};I.prototype.return=function(){this.l.s+='i';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,r='';for(const [a,b] of [new X(l)])r=a+':'+b+'|'+l.s;return r})()
// case: array binding stops stepping and skips return after early done
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.l.s+='n';this.i++;return new R(this.i,this.i>1)};I.prototype.return=function(){this.l.s+='i';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,r='';for(const [a,b,c] of [new X(l)])r=a+':'+b+':'+c+'|'+l.s;return r})()
// case: inner close throw closes the outer iterator and keeps its error
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.l.s+='n';return new R(++this.i,false)};I.prototype.return=function(){this.l.s+='i';throw 7};function Inner(l){this.l=l};Inner.prototype[Symbol.iterator]=function(){return new I(this.l)};function OI(l){this.i=0;this.l=l};OI.prototype.next=function(){return this.i++?new R(0,true):new R(new Inner(this.l),false)};OI.prototype.return=function(){this.l.s+='o';throw 9};function Outer(l){this.l=l};Outer.prototype[Symbol.iterator]=function(){return new OI(this.l)};var l=new L;try{for(const [a,b] of new Outer(l))0}catch(e){return e+'|'+l.s}})()
// case: inner next throw disables inner close but closes the outer iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.i=0;this.l=l};I.prototype.next=function(){this.l.s+='n';if(this.i++)throw 7;return new R(1,false)};I.prototype.return=function(){this.l.s+='i';throw 8};function Inner(l){this.l=l};Inner.prototype[Symbol.iterator]=function(){return new I(this.l)};function OI(l){this.i=0;this.l=l};OI.prototype.next=function(){return this.i++?new R(0,true):new R(new Inner(this.l),false)};OI.prototype.return=function(){this.l.s+='o';throw 9};function Outer(l){this.l=l};Outer.prototype[Symbol.iterator]=function(){return new OI(this.l)};var l=new L;try{for(const [a,b] of new Outer(l))0}catch(e){return e+'|'+l.s}})()
// case: assignment-target throw closes the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{for((null).value of new X(l))0}catch(e){return e.name+'|'+l.s}})()
// case: const write in the body closes the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{for(const v of new X(l))v=2}catch(e){return e.name+'|'+l.s}})()
// case: inner iterator closes before outer iterator on labelled break
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l,n){this.l=l;this.n=n};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+=this.n;return new R(0,true)};function X(l,n){this.l=l;this.n=n};X.prototype[Symbol.iterator]=function(){return new I(this.l,this.n)};var l=new L;outer:for(var a of new X(l,'o'))for(var b of new X(l,'i'))break outer;return l.s})()
// case: labelled continue closes only the exited inner iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l,n,max){this.l=l;this.n=n;this.max=max;this.i=0};I.prototype.next=function(){this.i++;return new R(this.i,this.i>this.max)};I.prototype.return=function(){this.l.s+=this.n;return new R(0,true)};function X(l,n,max){this.l=l;this.n=n;this.max=max};X.prototype[Symbol.iterator]=function(){return new I(this.l,this.n,this.max)};var l=new L;outer:for(var a of new X(l,'o',2))for(var b of new X(l,'i',9))continue outer;return l.s})()
// case: nested return closes iterators from inner to outer
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l,n){this.l=l;this.n=n};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+=this.n;return new R(0,true)};function X(l,n){this.l=l;this.n=n};X.prototype[Symbol.iterator]=function(){return new I(this.l,this.n)};var l=new L,value=(function(){for(var a of new X(l,'o'))for(var b of new X(l,'i'))return 5})();return value+'|'+l.s})()
// case: switch break inside the body does not close the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l;this.i=0};I.prototype.next=function(){this.i++;return new R(this.i,this.i>1)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,s='';for(var v of new X(l)){switch(v){case 1:break}s+='b'}return s+'|'+l.s})()
// case: inner block label break does not close the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l;this.i=0};I.prototype.next=function(){this.i++;return new R(this.i,this.i>1)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,s='';for(var v of new X(l)){inside:{break inside}s+='b'}return s+'|'+l.s})()
// case: finally inside loop runs before iterator close
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;for(var v of new X(l)){try{break}finally{l.s+='f'}}return l.s})()
// case: loop inside finally region closes before outer finally
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{for(var v of new X(l))break}finally{l.s+='f'}return l.s})()
// case: inner finally runs before iterator close on return
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,value=(function(){for(var v of new X(l)){try{return 5}finally{l.s+='f'}}})();return value+'|'+l.s})()
// case: return inside finally discards its gosub address and closes the iterator
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,value=(function(){for(var v of new X(l)){try{return 4}finally{return 5}}})();return value+'|'+l.s})()
// case: iterator closes before an outer finally on return
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L,value=(function(){try{for(var v of new X(l))return 5}finally{l.s+='f'}})();return value+'|'+l.s})()
// ERROR_CASES
// case: number is not iterable
for(var value of 1)value
// case: RegExp literal is not iterable
for(var value of /a;b/)value
// case: non-callable iterator method
function X(){}X.prototype[Symbol.iterator]=1;for(var value of new X)value
// case: iterator method must return an object
function X(){}X.prototype[Symbol.iterator]=function(){return 1};for(var value of new X)value
// case: next method must be callable
function I(){this.next=1}function X(){}X.prototype[Symbol.iterator]=function(){return new I};for(var value of new X)value
// case: next must return an object
function I(){}I.prototype.next=function(){return 1};function X(){}X.prototype[Symbol.iterator]=function(){return new I};for(var value of new X)value
// case: close result must be an object on break
function R(v,d){this.value=v;this.done=d};function I(){};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){return 1};function X(){};X.prototype[Symbol.iterator]=function(){return new I};for(var value of new X)break
// ARRAY_BINDING_CASES
// case: ordinary declarations share flat array binding lowering
(function(){var [a,,b,]=[1,2,3],c=4,[d]=[5];let [e]=[6],f=7;const []=[8],[g]=[9];return a+'|'+b+'|'+c+'|'+d+'|'+e+'|'+f+'|'+g})()
// case: classic for initializer accepts a flat lexical array binding
(function(){var s='';for(let [a,b]=[1,2];a<3;a++)s+=a+':'+b+'|';return s})()
// case: classic for flat bindings retain per-iteration captured cells
(function(){var closures=[];for(let [i,j]=[0,9];i<3;i++)closures.push(function(){return i+':'+j});return closures[0]()+'|'+closures[1]()+'|'+closures[2]()})()
// case: classic for element defaults accept in for lexical and var bindings
(function(){var result='';for(let [x=0 in {}]=[];;){result+=x;break}for(var [y=1 in {1:2}]=[];;){result+='|'+y;break}return result})()
// case: classic for whole-pattern initializer restores the In grammar
(function(){var result;for(let [x]=1 in {1:1}?[42]:[0];(result=x,false);){}return result})()
// case: block and switch declarations share the flat binding lowering
(function(){var s='';{let [a]=[1];s+=a}switch(0){case 0:const [b]=[2];s+=b}return s})()
// case: defaults run only for undefined and infer anonymous function names
(function(){let [a=9,b=8,c=function(){}]=[undefined,null];return a+'|'+b+'|'+c.name})()
// case: defaults observe earlier initialized bindings
(function(){let [a=1,b=a,c=b]=[];return a+'|'+b+'|'+c})()
// case: declaration lookahead balances a regexp default containing a bracket
(function(){let [value=/]/]=[];return value.test(']')})()
// case: defaults observe later bindings in their temporal dead zone
(function(){try{let [a=b,b=2]=[]}catch(e){return e.name+'|'+e.message}})()
// case: right operand observes the declaration temporal dead zone
(function(){let value='outer';try{{let [value]=[value]}}catch(e){return e.name+'|'+e.message}})()
// case: rest drains the iterator into a fresh array
(function(){let [head,...tail]='A\uD83D\uDCA9B';return head+'|'+tail.join(',')+'|'+Array.isArray(tail)})()
// case: rest exhaustion suppresses iterator return
(function(){var log='';var iterable={[Symbol.iterator]:function(){var i=0;return{next:function(){log+='n';i++;return{value:i,done:i>2}},return:function(){log+='r';return{done:true}}}}};let [...values]=iterable;return values.join(',')+'|'+log})()
// case: for-of array binding composes defaults and rest
(function(){var result='';for(const [head=1,...tail] of [[],[2,3,4]])result+=head+':'+tail.join(',')+'|';return result})()
// case: for-of default throw closes inner before outer
(function(){var log='',outerDone=false;var inner={[Symbol.iterator]:function(){return this},next:function(){log+='n';return{value:undefined,done:false}},return:function(){log+='i';return{done:true}}};var outer={[Symbol.iterator]:function(){return this},next:function(){if(outerDone)return{done:true};outerDone=true;return{value:inner,done:false}},return:function(){log+='o';return{done:true}}};try{for(const [value=(log+='d',function(){throw 7})(),...rest] of outer){}}catch(e){return e+'|'+log}})()
// case: nested pattern default runs before inner acquisition and closes inner then outer
(function(){var log='',outerDone=false,innerDone=false;var outer={[Symbol.iterator]:function(){log+='O';return this},next:function(){log+='o';if(outerDone)return{done:true};outerDone=true;return{value:undefined,done:false}},return:function(){log+='R';return{done:true}}};var inner={[Symbol.iterator]:function(){log+='I';return this},next:function(){log+='i';if(innerDone)return{done:true};innerDone=true;return{value:1,done:false}},return:function(){log+='r';return{done:true}}};let [[a]=inner]=outer;return a+'|'+log})()
// case: nested normal close throw wins while the outer iterator still closes
(function(){var log='',outerDone=false;var inner={[Symbol.iterator]:function(){log+='I';return this},next:function(){log+='i';return{value:1,done:false}},return:function(){log+='r';throw 'inner-close'}};var outer={[Symbol.iterator]:function(){log+='O';return this},next:function(){log+='o';if(outerDone)return{done:true};outerDone=true;return{value:inner,done:false}},return:function(){log+='R';throw 'outer-close'}};try{let [[a]]=outer}catch(e){return e+'|'+log}})()
// case: nested next throw skips inner return but still closes the outer iterator
(function(){var log='',outerDone=false;var inner={[Symbol.iterator]:function(){log+='I';return this},next:function(){log+='i';throw 'inner-next'},return:function(){log+='r';return{done:true}}};var outer={[Symbol.iterator]:function(){log+='O';return this},next:function(){log+='o';if(outerDone)return{done:true};outerDone=true;return{value:inner,done:false}},return:function(){log+='R';throw 'outer-close'}};try{let [[a]]=outer}catch(e){return e+'|'+log}})()
// case: empty pattern acquires and closes without stepping
(function(){var log='';var iterable={[Symbol.iterator]:function(){log+='i';return{next:function(){log+='n';return{value:1,done:false}},return:function(){log+='r';return{done:true}}}}};const []=iterable;return log})()
// case: elisions step once per hole and close a live iterator
(function(){var log='';var iterable={[Symbol.iterator]:function(){var i=0;return{next:function(){log+='n';return{value:++i,done:false}},return:function(){log+='r';return{done:true}}}}};let [,,value]=iterable;return value+'|'+log})()
// case: default throw closes the iterator and wins over close throw
(function(){var log='';var iterable={[Symbol.iterator]:function(){return{next:function(){log+='n';return{value:undefined,done:false}},return:function(){log+='r';throw 9}}}};try{let [value=(log+='d',function(){throw 7})()]=iterable}catch(e){return e+'|'+log}})()
// case: default throw after iterator exhaustion does not close
(function(){var log='';var iterable={[Symbol.iterator]:function(){return{next:function(){log+='n';return{done:true}},return:function(){log+='r';return{done:true}}}}};try{let [value=(log+='d',function(){throw 7})()]=iterable}catch(e){return e+'|'+log}})()
// case: iterator next throw disables declaration close
(function(){var log='';var iterable={[Symbol.iterator]:function(){return{next:function(){log+='n';throw 7},return:function(){log+='r';return{done:true}}}}};try{let [value]=iterable}catch(e){return e+'|'+log}})()
// case: normal declaration close propagates a return throw
(function(){var iterable={[Symbol.iterator]:function(){return{next:function(){return{value:1,done:false}},return:function(){throw 8}}}};try{const [value]=iterable}catch(e){return e}})()
// case: var declaration prepares a with reference before iterator step
(function(){var value='outer',object={value:'object'},once=false;var iterable={[Symbol.iterator]:function(){return this},next:function(){if(once)return{done:true};once=true;delete object.value;return{value:7,done:false}},return:function(){return{done:true}}};with(object){var [value]=iterable}return value+'|'+object.value})()
// case: var rest keeps its prepared with reference while draining
(function(){var value='outer',object={value:'object'},index=0;var iterable={[Symbol.iterator]:function(){return this},next:function(){delete object.value;index++;return{value:index,done:index>2}},return:function(){return{done:true}}};with(object){var [...value]=iterable}return value+'|'+object.value.join(',')})()
// case: var for-of binding prepares a with reference before inner step
(function(){var value='outer',object={value:'object'},once=false;var iterable={[Symbol.iterator]:function(){return this},next:function(){if(once)return{done:true};once=true;delete object.value;return{value:7,done:false}},return:function(){return{done:true}}};with(object){for(var [value] of [iterable]){}}return value+'|'+object.value})()
// case: nested var binding prepares its with reference after inner acquisition and before step
(function(){var w={},x='outer';var inner={[Symbol.iterator]:function(){w.x='iter';return this},next:function(){delete w.x;return{value:7,done:false}},return:function(){return{done:true}}};with(w){var [[x]]=[inner]}return x+'|'+w.x})()
