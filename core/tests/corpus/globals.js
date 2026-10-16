print(typeof hoistedFn, typeof hoistedVar, hoistedVar);
var hoistedVar = 1;
function hoistedFn() { return 'fn'; }
print(hoistedFn(), hoistedVar);
implicit = 'made';
print(implicit, globalThis.implicit);
globalThis.viaObject = 9;
print(viaObject);
try { print(missingName); } catch (e) { print(e.name + ': ' + e.message); }
function strictAssign() { 'use strict'; undeclaredStrict = 1; }
try { strictAssign(); } catch (e) { print(e.name + ': ' + e.message); }
print(typeof undefined, undefined === void 0, NaN !== NaN, Infinity > 1e308);
undefined = 5;
print(undefined);
function redeclared() { return 1; }
function redeclared() { return 2; }
print(redeclared());
var counter = 0;
function bump() { counter = counter + 1; }
bump(); bump();
print(counter);
