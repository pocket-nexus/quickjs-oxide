// VALUE_CASES
// case: sloppy block lexical and Annex outer use distinct closures
(function(){var inside;{function block(){return block}inside=block}return (inside!==block)+'|'+(inside()===inside)+'|'+(block()===inside)})()
// case: strict block function stays lexical and mutable
(function(){'use strict';var inside;{function block(){return 3}inside=block;block=4}return typeof block+'|'+inside()})()
// case: skipped sloppy block still declares an undefined Annex outer
(function(){if(false){function skipped(){return 1}}return typeof skipped+'|'+delete skipped})()
// case: authored Annex closure does not copy a mutated block lexical
(function(){var inside;{block=8;function block(){}inside=block}return inside+'|'+typeof block+'|'+block.name})()
// case: sloppy duplicate keeps the last block child and first Annex child
(function(){var inside;{function duplicate(){return 1}function duplicate(){return 2}inside=duplicate}return inside()+'|'+duplicate()+'|'+(inside===duplicate)})()
// case: duplicate child recursion resolves through the last lexical slot
(function(){var inside;{function duplicate(){return duplicate}function duplicate(){return 2}inside=duplicate}return (duplicate()===inside)+'|'+inside()})()
// case: existing outer var is updated at the declaration source position
(function(){var target=7,get=function(){return target},before,after;{before=get();function target(){return 4}after=get()}return before+'|'+typeof after+'|'+after()})()
// case: simple parameter suppresses the sloppy Annex outer update
(function(parameter){var inside;{function parameter(){return 5}inside=parameter}return parameter+'|'+inside()})(1)
// case: arguments name suppresses the sloppy Annex outer update
(function(){var inside;{function arguments(){return 6}inside=arguments}return inside()})()
// case: explicit arguments parameter remains unchanged beside its block lexical
(function(arguments){var inside;{function arguments(){return 6}inside=arguments}return arguments+'|'+inside()})(7)
// case: named expression self is shadowed by the synthetic Annex root var
(function(){var original=function self(){{function self(){return 7}}return self};var replacement=original();return (replacement!==original)+'|'+replacement()})()
// case: block function captures a later lexical initialized before its call
(function(){{function laterRead(){return later}let later=8;return laterRead()}})()
// case: captured block functions and cells are fresh on loop reentry
(function(){var first,second,i=0;while(i<2){let value=i;function fresh(){return value}if(i===0)first=fresh;else second=fresh;i++}return (first!==second)+'|'+first()+'|'+second()})()
// case: switch enters one declaration scope after the discriminant and before case tests
(function(){var trace=typeof selected,inside;switch(0){case (trace+='|'+typeof selected,0):function selected(){return 9}inside=selected;break;case 1:function selected(){return 10}}return trace+'|'+inside()+'|'+selected()})()
// case: normal Function constructor block follows the same declaration path
Function("var inside;{function dynamicBlock(){return 11}inside=dynamicBlock}return inside()+'|'+dynamicBlock()+'|'+(inside===dynamicBlock)")()
// case: prior enclosing lexical suppresses the sloppy Annex outer update
(function(){let shadow=12;{function shadow(){return 13}}return shadow})()
// case: strict block generator declaration resumes through completion
(function(){'use strict';var saved;{function* blockGenerator(){yield 4}saved=blockGenerator}var iterator=saved();return iterator.next().value+'|'+iterator.next().done})()
// case: async block function stays lexical and has async function shape
(function(){var saved;{async function asyncBlock(){return 3}saved=asyncBlock}return typeof asyncBlock+'|'+Object.prototype.toString.call(saved)+'|'+('prototype' in saved)})()
// ERROR_CASES
// case: entry-created block function observes a later lexical TDZ
(function(){{return readBeforeInitialization();function readBeforeInitialization(){return later}let later=1}})()
// case: Program Annex update observes a later global lexical TDZ
{function laterGlobalLexical(){}}let laterGlobalLexical;
// case: failed block lexical initializer preserves the escaped function cell
(function(){{function escapedBlock(){return later}Function.savedBlock=escapedBlock;let later=missingBlockInitializer}})()
// ---
Function.savedBlock()
// case: Annex global var precedes a later global lexical
{function orderedGlobalCollision(){}}let orderedGlobalCollision;
// ---
typeof globalThis.orderedGlobalCollision
// ---
orderedGlobalCollision
