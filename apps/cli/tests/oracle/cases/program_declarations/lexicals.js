// case: basic Program lexical environment
let local=1,named=function(){};const fixed=2;local+=2;typeof local+'|'+local+'|'+fixed+'|'+named.name+'|'+typeof globalThis.local+'|'+delete local+'|'+delete fixed
// case: strict Program lexicals remain global lexicals
'use strict';let local=1;const fixed=2;local+'|'+fixed+'|'+typeof globalThis.local
// case: Program lexical capture and mutation
let value=1,read=function(){return value},write=function(next){value=next};write(7);read()+'|'+value
// case: nested block shadows a Program lexical
let value=1;{let value=2;Function.blockValue=value;}value+'|'+Function.blockValue
// case: dynamic Function resolves an initialized Program lexical
let value=5;Function('return value')()
// case: let without initializer becomes undefined
let value;typeof value+'|'+(value===undefined)
// case: Program lexical flat array bindings stay off the global object
let [first=1,...rest]=[];const [third]=[3];first+'|'+rest.length+'|'+third+'|'+typeof globalThis.first+'|'+typeof globalThis.rest+'|'+typeof globalThis.third
// case: Program lexical nested array bindings stay off the global object
let [[first]=[40],...[second,third]]=[undefined,1,2];const [[fourth]]=[[2]];first+second+third+fourth+'|'+typeof globalThis.first
// case: Program lexical object bindings support fixed computed and recursive properties
let objectKey='computed',{fixed:objectFixed,[objectKey]:objectComputed}={fixed:1,computed:2};const {nested:{value:objectNested=3}}={nested:{}};objectFixed+'|'+objectComputed+'|'+objectNested+'|'+typeof globalThis.objectFixed+'|'+typeof globalThis.objectComputed+'|'+typeof globalThis.objectNested
// case: Program lexical object rest excludes consumed keys and stays off the global object
let {fixed,...objectRest}={fixed:1,value:2};fixed+'|'+objectRest.value+'|'+('fixed' in objectRest)+'|'+typeof globalThis.objectRest
// case: declaration preserves an earlier script completion
9;let value=1
// case: persistent mutable and const bindings
let crossValue=40;const crossFixed=2;Function.saved=function(){return ++crossValue+crossFixed};undefined
// ---
Function.saved()+'|'+Function.saved()+'|'+crossValue+'|'+crossFixed+'|'+typeof globalThis.crossValue
// ---
crossFixed=3
// ---
let crossValue=9
// case: existing global property splits from the lexical cell
globalThis.splitValue=10;Function.splitRead=function(){return splitValue};undefined
// ---
let splitValue=1
// ---
Function.splitRead()+'|'+splitValue+'|'+globalThis.splitValue
// ---
delete globalThis.splitValue
// ---
Function.splitRead()+'|'+splitValue+'|'+typeof globalThis.splitValue
// case: hidden unresolved VarRef becomes the Program lexical cell
Function.hiddenRead=function(){return hiddenValue};Function.hiddenWrite=function(next){hiddenValue=next};undefined
// ---
let hiddenValue=1
// ---
Function.hiddenWrite(2);hiddenValue+'|'+Function.hiddenRead()+'|'+typeof globalThis.hiddenValue
// case: auto-init global property stays separate from the Program lexical cell
let Number=1
// ---
Number+'|'+typeof globalThis.Number
// ---
Number=2;Number
// ---
delete Number
// case: failed initializer keeps the pinned poisoned lexical
Function.failedRead=function(){return failedValue};let failedValue=(function(){throw 17})()
// ---
Function.failedRead()
// ---
failedValue
// ---
typeof failedValue
// ---
delete failedValue
// ---
let failedValue=1
// case: poisoned lexical distinguishes typed and precompiled global descriptors
Function.oldPoisonedRead=function(){return poisonedValue};Function.oldPoisonedWrite=function(next){poisonedValue=next};undefined
// ---
Function.declaredPoisonedRead=function(){return poisonedValue};let poisonedValue=(function(){throw 17})()
// ---
Function.declaredPoisonedRead()
// ---
Function.oldPoisonedRead()
// ---
Function.oldPoisonedWrite(1)
// ---
poisonedValue
// ---
typeof poisonedValue
// ---
delete poisonedValue
// ---
let poisonedValue=2
// case: partial initialization preserves initialized and poisoned cells
Function.readPartialFirst=function(){return partialFirst};Function.readPartialSecond=function(){return partialSecond};let partialFirst=1,partialSecond=(function(){throw 19})()
// ---
Function.readPartialFirst()+'|'+partialFirst
// ---
Function.readPartialSecond()
// ---
partialSecond
// ---
typeof partialSecond
// ---
let partialFirst=2
// ---
let partialSecond=2
// case: parse failure creates no Program binding
let parseOnly=1;let parseOnly=2
// ---
typeof parseOnly
// ---
let parseOnly=3;parseOnly
// case: preflight conflict is atomic and source ordered
let untouched=function(){return Infinity},NaN=1,Infinity=2
// ---
typeof untouched
// ---
let untouched=4;untouched
// case: parse and preflight failures prevent authored side effects
globalThis.atomicMarker=0
// ---
atomicMarker=1;let parseResidue=1;let parseResidue=2
// ---
atomicMarker+'|'+typeof parseResidue
// ---
atomicMarker=2;let preflightResidue=(atomicMarker=3),NaN=1
// ---
atomicMarker+'|'+typeof preflightResidue
// ---
let parseResidue=4,preflightResidue=5;parseResidue+'|'+preflightResidue+'|'+atomicMarker
