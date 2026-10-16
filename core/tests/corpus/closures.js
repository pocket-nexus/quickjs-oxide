function counter() { var n = 0; return function () { return ++n; }; }
var c1 = counter(), c2 = counter();
print(c1(), c1(), c1(), c2());
function pair() { var v = 0; return { get: function () { return v; }, set: function (x) { v = x; } }; }
var p = pair(); p.set(42); print(p.get());
function outer(a) { var b = 1; return function () { return function () { return a + b++; }; }; }
var inner = outer(10)(); print(inner(), inner(), inner());
function adder(x) { return y => z => x + y + z; }
print(adder(1)(2)(3));
function captureParam(x) { var f = () => x; x = 'changed'; return f(); }
print(captureParam('orig'));
var fns = [];
function make(i) { return function () { return i * 10; }; }
for (var i = 0; i < 3; i++) fns[i] = make(i);
print(fns[0](), fns[1](), fns[2]());
function recurse() { var self = function (n) { return n <= 1 ? 1 : n * self(n - 1); }; return self(10); }
print(recurse());
var fact = function f(n) { return n <= 1 ? 1 : n * f(n - 1); };
print(fact(12));
function shadow() { var x = 'outer'; { let x = 'block'; var g = () => x; } return g() + ' ' + x; }
print(shadow());
function hoisted() { return h(); function h() { return 'hoisted'; } }
print(hoisted());
let late = () => lateValue;
let lateValue = 'late';
print(late());
