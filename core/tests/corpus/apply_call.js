// `Function.prototype.apply` and `call` inside the loop.
function show(a, b) { return this.name + ':' + a + ',' + b; }
var self = { name: 'self' };
print(show.call(self, 1, 2), show.call(self), show.apply(self, [3, 4]));
print(show.apply(self), show.apply(self, null), show.apply(self, undefined));
function args() { return show.apply(self, arguments); }
print(args(5, 6), args());
var arrayLike = { length: 2, 0: 'x', 1: 'y' };
print(show.apply(self, arrayLike));
function Base(v) { this.v = v; this.kind = 'base'; }
function Derived(v, w) { Base.call(this, v); this.w = w; }
var d = new Derived(1, 2);
print(d.v, d.w, d.kind);
function Klass() { this.initialize.apply(this, arguments); }
Klass.prototype.initialize = function (a, b) { this.sum = a + b; };
var k = new Klass(3, 4);
print(k.sum);
for (var i = 0; i < 50; i++) k = new Klass(i, i);
print(k.sum);
function sloppy() { return this === globalThis; }
function strict() { 'use strict'; return this; }
print(sloppy.call(null), sloppy.call(undefined), strict.call(null), strict.call(7));
print(show.call.call(show, self, 'c', 'd'));
print(typeof show.apply, typeof show.call);
var max = function () { var m = -1; for (var j = 0; j < arguments.length; j++) if (arguments[j] > m) m = arguments[j]; return m; };
print(max.apply(null, [3, 9, 2]), max.call(null, 1, 7));
try { show.apply(self, 5); } catch (e) { print(e.name); }
try { var c = show.call; c(); } catch (e) { print(e.name); }
print.apply(null, ['native', 'via', 'apply']);
print.call(null, 'native', 'via', 'call');
function spreadToNative() { print(...arguments); }
spreadToNative('native', 'via', 'spread');
var many = []; for (var n = 0; n < 300; n++) many[n] = n;
print(max.apply(null, many), max(...many));
