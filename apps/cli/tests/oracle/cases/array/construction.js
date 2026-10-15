// CONSTRUCTOR_AND_LENGTH_CASES
// case: constructor metadata and isArray
(function(){function X(){};return typeof Array+'|'+Array.name+'|'+Array.length+'|'+(Array.prototype.constructor===Array)+'|'+Array.isArray([])+'|'+Array.isArray(new X)})()
// case: call and construct empty arrays
(function(){var a=Array(),b=new Array;return a.length+'|'+b.length+'|'+(a instanceof Array)+'|'+(b instanceof Array)})()
// case: single numeric argument creates holes
(function(){var a=Array(3);return a.length+'|'+(0 in a)+'|'+(1 in a)+'|'+(2 in a)})()
// case: single string argument is an element
(function(){var a=Array('3');return a.length+'|'+a[0]+'|'+(0 in a)})()
// case: multiple constructor arguments become dense elements
(function(){var a=new Array(4,5,6);return a.length+'|'+a[0]+'|'+a[1]+'|'+a[2]})()
// case: single object argument preserves identity
(function(){function X(){this.marker=7};var x=new X,a=Array(x);return a.length+'|'+(a[0]===x)+'|'+a[0].marker})()
// case: indexed assignment grows length
(function(){var a=[];a[2]=7;return a.length+'|'+(0 in a)+'|'+(1 in a)+'|'+(2 in a)+'|'+a[2]})()
// case: delete creates a hole without shrinking length
(function(){var a=[1,2,3];var d=delete a[2];return d+'|'+a.length+'|'+(2 in a)+'|'+a[2]})()
// case: length shrink deletes higher indices
(function(){var a=[0,1,2,3];a.length=2;return a.length+'|'+a[0]+'|'+a[1]+'|'+(2 in a)+'|'+(3 in a)})()
// case: zero length truncates every indexed property
(function(){var a=[1,2];a.extra=3;a.length=0;return a.length+'|'+(0 in a)+'|'+(1 in a)+'|'+a.extra})()
// case: negative zero is a valid zero length
(function(){var a=[1];a.length=-0;return a.length+'|'+(1/a.length)+'|'+(0 in a)})()
// case: array-index boundary distinguishes 2^32 minus one
(function(){var a=[];a[4294967294]=7;a[4294967295]=8;return a.length+'|'+a[4294967294]+'|'+a[4294967295]})()
// case: Array.from consumes an iterable and maps with thisArg
(function(){function T(){this.bias=10};var a=Array.from([1,2],function(v,i){return this.bias+v+i},new T);return a.length+'|'+a[0]+'|'+a[1]})()
// case: Array.from falls back to array-like indexing
(function(){function X(){this[0]='x';this[2]='z';this.length=3};var a=Array.from(new X);return a.length+'|'+a[0]+'|'+a[1]+'|'+a[2]+'|'+(1 in a)})()
// case: Array.of and species preserve constructor identities
(function(){var a=Array.of(3,4);return a.length+'|'+a[0]+'|'+a[1]+'|'+(Array[Symbol.species]===Array)})()
// case: Array.from closes an iterator when its mapper throws
(function(){function R(v,d){this.value=v;this.done=d};function L(){this.s=''};function I(l){this.l=l};I.prototype.next=function(){return new R(1,false)};I.prototype.return=function(){this.l.s+='r';return new R(0,true)};function X(l){this.l=l};X.prototype[Symbol.iterator]=function(){return new I(this.l)};var l=new L;try{Array.from(new X(l),function(){throw 7})}catch(e){return e+'|'+l.s}})()
// case: hole lookup continues through Array prototype
(function(){Array.prototype[1]=9;var a=[0,,2];return a.length+'|'+a[1]+'|'+(1 in a)})()
// LITERAL_CASES
// case: dense array literal
(function(){var a=[1,2,3];return a.length+'|'+a[0]+'|'+a[1]+'|'+a[2]})()
// case: dense literal crosses the 32 element lowering boundary
(function(){var a=[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24,25,26,27,28,29,30,31,32,33,34,35,36,37,38,39];return a.length+'|'+a[0]+'|'+a[31]+'|'+a[32]+'|'+a[39]})()
// case: leading middle and trailing elisions
(function(){var a=[,1,,3,,];return a.length+'|'+(0 in a)+'|'+a[1]+'|'+(2 in a)+'|'+a[3]+'|'+(4 in a)})()
// case: trailing comma does not add an elision
(function(){var a=[1,],b=[1,,];return a.length+'|'+(1 in a)+'|'+b.length+'|'+(1 in b)})()
// case: nested arrays retain independent identity
(function(){var a=[[1],[2]];return a.length+'|'+a[0][0]+'|'+a[1][0]+'|'+(a[0]!==a[1])})()
// case: String spread advances by Unicode code point
(function(){var a=[0,...'A\uD83D\uDCA9\uD800',3];return a.length+'|'+a[0]+'|'+a[1]+'|'+a[2].length+'|'+a[2].charCodeAt(0)+'|'+a[3].charCodeAt(0)+'|'+a[4]})()
// case: spreading a sparse array materializes undefined
(function(){var source=[,2],a=[...source];return a.length+'|'+(0 in a)+'|'+a[0]+'|'+a[1]})()
// case: spread and later elision share one growing length
(function(){var a=[...'ab',,3,];return a.length+'|'+a[0]+'|'+a[1]+'|'+(2 in a)+'|'+a[3]})()
// case: custom iterable spread preserves value order
(function(){function R(v,d){this.value=v;this.done=d};function I(){this.i=0};I.prototype.next=function(){this.i++;return new R(this.i,this.i>2)};function X(){};X.prototype[Symbol.iterator]=function(){return new I};var a=[0,...new X,3];return a.length+'|'+a[0]+'|'+a[1]+'|'+a[2]+'|'+a[3]})()
// case: literal element expressions run left to right
(function(){Function.arrayLiteralLog='';function f(v){Function.arrayLiteralLog+=v;return v};var a=[f(1),f(2),f(3)];return a[0]+a[1]+a[2]+'|'+Function.arrayLiteralLog})()
// ITERATOR_CASES
// case: for-of visits values and holes
(function(){var s='';for(var v of [1,,3])s+=typeof v+':'+v+'|';return s})()
// case: values iterator observes holes as undefined
(function(){var i=[10,,30].values(),a=i.next(),b=i.next(),c=i.next(),d=i.next();return a.done+':'+a.value+'|'+b.done+':'+b.value+'|'+c.done+':'+c.value+'|'+d.done+':'+d.value})()
// case: keys iterator visits every index below length
(function(){var i=Array(3).keys(),a=i.next(),b=i.next(),c=i.next(),d=i.next();return a.value+'|'+b.value+'|'+c.value+'|'+d.done+'|'+d.value})()
// case: entries iterator returns fresh key value arrays
(function(){var i=[4,,6].entries(),a=i.next(),b=i.next(),c=i.next();return Array.isArray(a.value)+'|'+a.value[0]+':'+a.value[1]+'|'+b.value[0]+':'+b.value[1]+'|'+c.value[0]+':'+c.value[1]+'|'+(a.value!==b.value)})()
// case: values is the default iterator and iterator is self iterable
(function(){var a=[],i=a[Symbol.iterator]();return (a.values===a[Symbol.iterator])+'|'+(i[Symbol.iterator]()===i)+'|'+i[Symbol.toStringTag]})()
// case: detached next works with its branded receiver
(function(){var i=[7].values(),next=i.next,a=next.call(i),b=next.call(i);return a.done+'|'+a.value+'|'+b.done+'|'+b.value})()
// case: completed iterator remains completed after append
(function(){var a=[1],i=a.values();i.next();var before=i.next();a[1]=2;var after=i.next();return before.done+'|'+before.value+'|'+after.done+'|'+after.value})()
// case: iterator observes append before completion
(function(){var a=[1],i=a.values(),first=i.next();a[1]=2;var second=i.next(),done=i.next();return first.value+'|'+second.value+'|'+done.done})()
// case: iterator observes length shrink before the next step
(function(){var a=[1,2,3],i=a.values(),first=i.next();a.length=1;var done=i.next();return first.value+'|'+done.done+'|'+done.value})()
// case: iterator observes deletion as undefined
(function(){var a=[1,2],i=a.values(),first=i.next();delete a[1];var second=i.next();return first.value+'|'+second.done+'|'+second.value+'|'+(1 in a)})()
// case: Array iterator methods are generic over array-like objects
(function(){function X(){this[0]='x';this[2]='z';this.length=3};var i=Array.prototype.values.call(new X),a=i.next(),b=i.next(),c=i.next(),d=i.next();return a.value+'|'+b.value+'|'+c.value+'|'+d.done})()
// case: for-of captures a fresh lexical binding for each array element
(function(){var f,g,n=0;for(let value of [4,5]){n++;if(n===1)f=function(){return value};else g=function(){return value}}return f()+'|'+g()})()
// ERROR_CASES
// case: negative constructor length
Array(-1)
// case: fractional constructor length
Array(1.5)
// case: NaN constructor length
Array(0/0)
// case: 2^32 constructor length
Array(4294967296)
// case: negative assigned length
(function(){var a=[];a.length=-1;return a})()
// case: fractional assigned length
(function(){var a=[];a.length=1.5;return a})()
// case: Symbol assigned length
(function(){var a=[];a.length=Symbol();return a})()
// case: non iterable literal spread
[...1]
// case: non callable spread iterator method
(function(){function X(){};X.prototype[Symbol.iterator]=1;return [...new X]})()
// case: iterator next requires an Array Iterator receiver
(function(){function X(){};var next=[].values().next;return next.call(new X)})()
