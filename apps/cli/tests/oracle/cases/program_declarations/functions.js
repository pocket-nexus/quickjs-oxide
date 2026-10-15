// case: Program function is callable before its declaration
typeof basicFunction+'|'+basicFunction();function basicFunction(){return 7}
// case: strict Program function is callable before its declaration
'use strict';typeof strictFunction+'|'+strictFunction();function strictFunction(){return 8}
// case: last duplicate Program function wins before authored code
duplicateFunction();function duplicateFunction(){return 1}function duplicateFunction(){return 2}
// case: strict duplicate Program functions remain valid
'use strict';duplicateStrict();function duplicateStrict(){return 1}function duplicateStrict(){return 2}
// case: Program var initializer runs after the last function hoist
var firstMixed=mixedFunction();function mixedFunction(){return 1}var middleMixed=mixedFunction();var mixedFunction=function(){return 3};function mixedFunction(){return 2}firstMixed+'|'+middleMixed+'|'+mixedFunction()
// case: Program var without initializer preserves the function hoist
typeof varFunction+'|'+varFunction();var varFunction;function varFunction(){return 4}
// case: Program function preserves an earlier script completion
9;function completionFunction(){return 1}
// case: Program function metadata and self reference use its declared name
function metadataFunction(first,second){return metadataFunction===globalThis.metadataFunction}metadataFunction.name+'|'+metadataFunction.length+'|'+metadataFunction()+'|'+(metadataFunction.prototype.constructor===metadataFunction)
// case: Program declaration name is mutable rather than a private function-name binding
'use strict';function mutableDeclaration(){mutableDeclaration=3;return mutableDeclaration}mutableDeclaration()+'|'+globalThis.mutableDeclaration
// case: Program function captures a later lexical binding
function readLaterLexical(){return laterLexical}let laterLexical=5;readLaterLexical()
// case: lexical-first quirk exposes the hoisted function before let initialization
Function.savedLexicalFirst=lexicalFirst;let lexicalFirst=1;function lexicalFirst(){return 2}Function.savedLexicalFirst()+'|'+lexicalFirst+'|'+typeof globalThis.lexicalFirst
// case: const-first quirk exposes the hoisted function before const initialization
Function.savedConstFirst=constFirst;const constFirst=1;function constFirst(){return 2}Function.savedConstFirst()+'|'+constFirst
// case: const self initializer reads the raw-hoisted function
const constSelf=constSelf;function constSelf(){return 6}constSelf()
// case: last duplicate function hoists into a preceding lexical cell
Function.savedDuplicateLexical=duplicateLexical;let duplicateLexical=1;function duplicateLexical(){return 2}function duplicateLexical(){return 3}Function.savedDuplicateLexical()+'|'+duplicateLexical
// case: Program function prologue preserves authored branch targets
if(false){missingFunctionBranch}else{Function.functionBranch=1}function branchFunction(){return 4}Function.functionBranch+'|'+branchFunction()
// case: Program generator declaration hoists and resumes through completion
function* programGenerator(){yield 1;return 2}var iterator=programGenerator();iterator.next().value+'|'+iterator.next().value+'|'+iterator.next().done
// case: async Program declaration hoists with async function shape
typeof asyncProgram+'|'+Object.prototype.toString.call(asyncProgram)+'|'+asyncProgram.name+'|'+asyncProgram.length+'|'+('prototype' in asyncProgram);async function asyncProgram(value){return value}
// case: later Program function declaration replaces the global function identity
function crossFunction(){return 1}Function.oldCrossFunction=crossFunction;undefined
// ---
crossFunction()
// ---
function crossFunction(){return 2}crossFunction()
// ---
Function.oldCrossFunction()+'|'+crossFunction()+'|'+(Function.oldCrossFunction===crossFunction)
// case: configurable property becomes a fixed Program function binding
globalThis.configFunction=1
// ---
function configFunction(){return 3}configFunction()
// ---
delete configFunction
// ---
let configFunction=4
// ---
configFunction()
// case: existing global lexical blocks a later Program function
let lexicalFunctionConflict=1
// ---
function lexicalFunctionConflict(){return 2}
// ---
lexicalFunctionConflict+'|'+typeof globalThis.lexicalFunctionConflict
// case: Program var and function share one global cell across evals
var varFunctionConflict=1
// ---
function varFunctionConflict(){return 2}varFunctionConflict()
// ---
var varFunctionConflict
// ---
varFunctionConflict()
// ---
var varFunctionConflict=4
// ---
varFunctionConflict
// case: Program function closure retains a later lexical binding
function persistentLexicalRead(){return persistentLexical}let persistentLexical=5;persistentLexicalRead()
// ---
persistentLexicalRead()+'|'+persistentLexical
// case: failed lexical initializer preserves its raw-hoisted function
let failedLexicalFunction=missingLexicalInitializer;function failedLexicalFunction(){return 9}
// ---
typeof failedLexicalFunction+'|'+failedLexicalFunction()+'|'+typeof globalThis.failedLexicalFunction
