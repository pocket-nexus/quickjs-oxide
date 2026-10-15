// case: Program var without initializer
var empty;typeof empty+'|'+(empty===undefined)+'|'+typeof globalThis.empty+'|'+delete empty
// case: duplicate Program var initializers run in source order
globalThis.varLog='';var repeated=(varLog+='a',1);var repeated=(varLog+='b',repeated+1);varLog+'|'+repeated
// case: Program var has no TDZ
var selfValue=selfValue;typeof selfValue
// case: Program var initializer performs NamedEvaluation
var named=function(){};named.name
// case: Program var flat array binding publishes global cells
var [first,,third=3,...rest]=[1,2,undefined,4,5];first+'|'+third+'|'+rest.join(',')+'|'+globalThis.first+'|'+globalThis.third+'|'+globalThis.rest.length
// case: Program var nested array binding supports defaults and rest patterns
var [[first]=[40],...[second,third]]=[undefined,1,2];first+'|'+second+'|'+third+'|'+globalThis.first
// case: Program var object binding supports fixed computed and recursive properties
var objectKey='computed';var {fixed:objectFixed,[objectKey]:objectComputed,nested:{value:objectNested=3}}={fixed:1,computed:2,nested:{}};objectFixed+'|'+objectComputed+'|'+objectNested+'|'+globalThis.objectFixed+'|'+globalThis.objectComputed+'|'+globalThis.objectNested
// case: Program var object rest excludes consumed keys and publishes its global cell
var {fixed,...objectRest}={fixed:1,value:2};fixed+'|'+objectRest.value+'|'+('fixed' in objectRest)+'|'+(globalThis.objectRest===objectRest)
// case: Program var preserves an earlier script completion
9;var completed=1
// case: sloppy Program var accepts contextual let
var let=1;let
// case: unreached blocks and switch clauses still hoist Program vars
if(false){var blocked=1}switch(0){case 1:var switched=2}typeof blocked+'|'+typeof switched
// case: classic for Program var uses one shared global cell
var readLoop=function(){return loopValue};for(var loopValue=0;loopValue<3;loopValue++);loopValue+'|'+readLoop()
// case: nested block lexical shadows a Program var
var outerValue=1;{let outerValue=2;Function.blockValue=outerValue}outerValue+'|'+Function.blockValue
// case: repeated Program var preserves and updates one global cell
var crossValue=1;Function.crossRead=function(){return crossValue};undefined
// ---
var crossValue
// ---
crossValue+'|'+Function.crossRead()
// ---
var crossValue=2;crossValue+'|'+Function.crossRead()
// ---
let crossValue=3
// case: hidden unresolved VarRef reconnects to a later Program var
Function.hiddenVarRead=function(){return hiddenVar};undefined
// ---
var hiddenVar=4
// ---
Function.hiddenVarRead()+'|'+hiddenVar+'|'+typeof globalThis.hiddenVar
// case: configurable global property survives var and reconnects after delete
globalThis.reconnectVar=1;Function.reconnectVarRead=function(){return reconnectVar};undefined
// ---
var reconnectVar=2
// ---
Function.reconnectVarRead()+'|'+reconnectVar+'|'+globalThis.reconnectVar
// ---
delete reconnectVar
// ---
Function.reconnectVarRead()
// ---
reconnectVar=3
// ---
Function.reconnectVarRead()+'|'+reconnectVar+'|'+globalThis.reconnectVar
// case: var preserves a configurable property so a later lexical may split
globalThis.splitAfterVar=1
// ---
var splitAfterVar=2
// ---
let splitAfterVar=3
// ---
splitAfterVar+'|'+globalThis.splitAfterVar
// ---
var splitAfterVar=4
// ---
splitAfterVar+'|'+globalThis.splitAfterVar
// case: Program var preflight conflict is atomic and source ordered
globalThis.varMarker=0;let existingVarConflict=9
// ---
varMarker=1;var freshVarBefore=(varMarker=2),existingVarConflict=(varMarker=3),freshVarAfter=(varMarker=4)
// ---
varMarker+'|'+typeof freshVarBefore+'|'+typeof freshVarAfter+'|'+existingVarConflict
// ---
var freshVarBefore=5,freshVarAfter=6;freshVarBefore+'|'+freshVarAfter
// case: abrupt initializer leaves every Program var instantiated
globalThis.initializerLog='';Function.readFirstVar=function(){return firstVar};Function.readSecondVar=function(){return secondVar};Function.readThirdVar=function(){return thirdVar};var firstVar=(initializerLog+='a',1),secondVar=(initializerLog+='b',(function(){throw 17})()),thirdVar=(initializerLog+='c',3)
// ---
initializerLog+'|'+Function.readFirstVar()+'|'+typeof Function.readSecondVar()+'|'+typeof Function.readThirdVar()
// ---
var secondVar=2,thirdVar=3;firstVar+'|'+secondVar+'|'+thirdVar
