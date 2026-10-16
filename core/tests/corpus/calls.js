function fib(n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
print(fib(20));
function even(n) { return n === 0 ? true : odd(n - 1); }
function odd(n) { return n === 0 ? false : even(n - 1); }
print(even(100), odd(7));
function args(a, b, c) { return a + ',' + b + ',' + c; }
print(args(1), args(1, 2, 3, 4), args());
function locals(a) { var x; var y = a; return x + ',' + y; }
print(locals(1, 2, 3));
function sloppyThis() { return this === globalThis; }
function strictThis() { 'use strict'; return this; }
print(sloppyThis(), strictThis(), typeof strictThis());
var obj = { name: 'obj', who: function () { return this.name; } };
print(obj.who(), obj['who']());
var detached = obj.who;
print(detached());
print((function (x) { return x * 2; })(21), (x => x + 1)(1));
function apply(f, v) { return f(v); }
print(apply(function (v) { return v + '!'; }, 'hi'));
function noReturn() {}
print(noReturn(), typeof noReturn());
var calls = 0;
function count() { calls++; return calls; }
for (var i = 0; i < 1000; i++) count();
print(calls);
function manyArgs(a, b, c, d, e, f, g, h) { return a + b + c + d + e + f + g + h; }
print(manyArgs(1, 2, 3, 4, 5, 6, 7, 8));
try { var notFn = 5; notFn(); } catch (e) { print(e instanceof TypeError, e.message); }
try { ({}).missing(); } catch (e) { print(e instanceof TypeError); }
function depth(n) { return n === 0 ? 0 : 1 + depth(n - 1); }
print(depth(3000));
