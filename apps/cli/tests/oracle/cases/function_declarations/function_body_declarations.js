// VALUE_CASES
// case: direct body declaration is callable before its source position
(function bodyHoist(){var first=bodyTarget();return typeof bodyTarget+'|'+first+'|'+bodyTarget.name+'|'+bodyTarget.length+'|'+(bodyTarget.prototype.constructor===bodyTarget);function bodyTarget(left,right){return 7}})()
// case: each function activation creates a fresh hoisted closure
(function(){function make(){return local;function local(){}}return make()!==make()})()
// case: last duplicate body declaration wins in sloppy code
(function(){return duplicate();function duplicate(){return 1}function duplicate(){return 2}})()
// case: last duplicate body declaration wins in strict code
(function(){'use strict';return duplicate();function duplicate(){return 3}function duplicate(){return 4}})()
// case: var without an initializer preserves the function hoist
(function(){var before=typeof mixed+'|'+mixed();var mixed;return before;function mixed(){return 5}})()
// case: var initializer runs after the function hoist
(function(){var before=mixed();var mixed=function(){return 9};return before+'|'+mixed();function mixed(){return 6}})()
// case: body declaration replaces a same-name parameter
(function(parameter){var original=parameter;function parameter(){return 7}return typeof original+'|'+original()})(3)
// case: body declaration replaces the last duplicate parameter slot
(function(parameter,parameter){function parameter(){return 13}return parameter()})(1,2)
// case: arguments-name declaration replaces the ordinary body binding
(function(){function arguments(){return 8}return typeof arguments+'|'+arguments.name+'|'+arguments()})()
// case: var arguments before its declaration shares the hoisted body local
(function(){var arguments;function arguments(){return 8}return arguments()})()
// case: var arguments after its declaration preserves the hoisted body local
(function(){function arguments(){return 8}var arguments;return arguments()})()
// case: var arguments initializer overwrites the declaration hoist
(function(){var arguments=function(){return 9};function arguments(){return 8}return arguments()})()
// case: body declaration shadows a named-expression self binding
(function(){var expression=function self(){function self(){return 'body'}return (self===expression)+'|'+self()};return expression()})()
// case: declaration self reference uses the mutable body cell
(function(){function mutable(){return mutable}var original=mutable;mutable=17;return (original()===17)+'|'+typeof mutable})()
// case: sibling body declarations capture each other
(function(){function first(){return second}function second(){return first}return first()===second&&second()===first})()
// case: hoisted declaration captures a later initialized lexical
(function(){function readLater(){return later}let later=42;return readLater()})()
// case: Function constructor body uses the direct declaration hoist path
Function('return dynamicTarget();function dynamicTarget(){return 12}')()
// case: direct body generator declaration hoists and resumes through completion
(function(){function* bodyGenerator(){yield 1;return 2}var iterator=bodyGenerator();return iterator.next().value+'|'+iterator.next().value+'|'+iterator.next().done})()
// case: direct body async declaration hoists with async function shape
(function(){var before=Object.prototype.toString.call(bodyAsync)+'|'+bodyAsync.name+'|'+bodyAsync.length+'|'+('prototype' in bodyAsync);async function bodyAsync(value){return value}return before})()
// ERROR_OBSERVATION_CASES
// case: hoisted declaration observes the later lexical temporal dead zone
(function bodyDeclarationTdz(){return readLater();function readLater(){return later}let later=42})()
// case: failed body lexical initializer preserves its escaped hoist cell
(function(){Function.savedBodyRead=readLater;let later=missingBodyInitializer;function readLater(){return later}})()
// ---
Function.savedBodyRead()
