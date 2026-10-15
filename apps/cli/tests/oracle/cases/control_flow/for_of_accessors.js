Function.forOfTrace='';
function ForOfResult(done,value,throwDone,throwValue){
    this.doneSlot=done;
    this.valueSlot=value;
    this.throwDone=throwDone;
    this.throwValue=throwValue;
}
function ForOfIterator(){this.i=0}
function ForOfCachedNext(){
    Function.forOfTrace+='c';
    this.i++;
    return new ForOfResult(this.i>this.limit,this.i,this.throwDone,this.throwValue);
}
function ForOfReturn(){
    Function.forOfTrace+='q';
    if(this.returnThrow)throw 10;
    if(this.returnPrimitive)return 1;
    return new ForOfResult(true,0,false,false);
}
var ForOfResultPrototype=Object.create(Object.prototype);
Object.defineProperty(ForOfResultPrototype,'done',{configurable:true,get:function(){Function.forOfTrace+='d';if(this.throwDone)throw 7;return this.doneSlot}});
Object.defineProperty(ForOfResultPrototype,'value',{configurable:true,get:function(){Function.forOfTrace+='v';if(this.throwValue)throw 8;return this.valueSlot}});
ForOfResult.prototype=ForOfResultPrototype;
var ForOfIteratorPrototype=Object.create(Object.prototype);
Object.defineProperty(ForOfIteratorPrototype,'next',{configurable:true,get:function(){Function.forOfTrace+='n';return ForOfCachedNext}});
Object.defineProperty(ForOfIteratorPrototype,'return',{configurable:true,get:function(){Function.forOfTrace+='r';if(this.returnGetterThrow)throw 9;if(this.returnNull)return null;if(this.returnUndefined)return undefined;if(this.returnNonCallable)return 1;return ForOfReturn}});
ForOfIterator.prototype=ForOfIteratorPrototype;
// case: done is read before value and a done result skips value
(function(){function X(){};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;return i};var s='';for(var v of new X)s+=v;return s+'|'+Function.forOfTrace})()
// case: done getter throw does not close the iterator
(function(){function X(){};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;i.throwDone=true;return i};try{for(var v of new X)v}catch(e){return e+'|'+Function.forOfTrace}})()
// case: value getter throw does not close the iterator
(function(){function X(){};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;i.throwValue=true;return i};try{for(var v of new X)v}catch(e){return e+'|'+Function.forOfTrace}})()
// case: next getter is cached before later prototype mutation
(function(){function X(){};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;return i};var s='';for(var v of new X){s+=v;delete ForOfIteratorPrototype.next;ForOfIteratorPrototype.next=function(){throw 11}}return s+'|'+Function.forOfTrace})()
// case: nullish return getters complete break normally
(function(){function X(mode){this.mode=mode};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;i.returnNull=this.mode===1;i.returnUndefined=this.mode===2;return i};for(var a of new X(1))break;for(var b of new X(2))break;return Function.forOfTrace})()
// case: return getter throw replaces break but not a pending throw
(function(){function X(){};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;i.returnGetterThrow=true;return i};var normal,pending;try{for(var a of new X)break}catch(e){normal=e};try{for(var b of new X)throw 7}catch(e){pending=e};return normal+'|'+pending+'|'+Function.forOfTrace})()
// case: non-callable return getter result replaces break but not a pending throw
(function(){function X(){};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;i.returnNonCallable=true;return i};var normal,pending;try{for(var a of new X)break}catch(e){normal=e.name};try{for(var b of new X)throw 7}catch(e){pending=e};return normal+'|'+pending+'|'+Function.forOfTrace})()
// case: primitive return call result replaces break but not a pending throw
(function(){function X(){};X.prototype[Symbol.iterator]=function(){var i=new ForOfIterator;i.limit=1;i.returnPrimitive=true;return i};var normal,pending;try{for(var a of new X)break}catch(e){normal=e.name};try{for(var b of new X)throw 7}catch(e){pending=e};return normal+'|'+pending+'|'+Function.forOfTrace})()
