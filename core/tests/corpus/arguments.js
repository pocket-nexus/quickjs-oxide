// `arguments`, mapped and unmapped (phase 3, section 5.1).
function count() { return arguments.length; }
print(count(), count(1), count(1, 2, 3), count(undefined));
function mapped(a, b) {
  arguments[0] = 'A';
  b = 'B';
  return a + ' ' + arguments[1] + ' ' + arguments.length + ' ' + arguments[2];
}
print(mapped(1, 2, 3), mapped(1));
function unmapped(a, b) {
  'use strict';
  arguments[0] = 'A';
  b = 'B';
  return a + ' ' + arguments[0] + ' ' + arguments[1] + ' ' + arguments.length;
}
print(unmapped(1, 2), unmapped());
function extras(a) {
  var s = '';
  for (var i = 0; i < arguments.length; i++) s += arguments[i] + ';';
  return s;
}
print(extras(), extras(1), extras(1, 2, 3, 4, 5));
function writeExtra(a) { arguments[2] = 'x'; return arguments[2] + arguments.length; }
print(writeExtra(1, 2, 3));
function deleted(a, b) {
  delete arguments[0];
  a = 'changed';
  return arguments[0] + ' ' + a + ' ' + (0 in arguments) + ' ' + (1 in arguments);
}
print(deleted(1, 2));
function missing(a, b) { b = 5; return arguments[1] + ' ' + arguments.length; }
print(missing(1));
function captured(a) {
  var get = function () { return a; };
  arguments[0] = 'via arguments';
  return get();
}
print(captured('orig'));
function calleeCheck() { return arguments.callee === calleeCheck; }
print(calleeCheck(), typeof arguments === 'undefined' ? 'none' : 'global');
function typeOfArgs() { return typeof arguments + ' ' + arguments; }
print(typeOfArgs(1));
function inner() {
  var outer = arguments;
  return (function () { return outer[0] + arguments[0]; })('b');
}
print(inner('a'));
function hot(a, b) { return arguments.length + a + b; }
var total = 0;
for (var k = 0; k < 100; k++) total += hot(k, 1);
print(total);
