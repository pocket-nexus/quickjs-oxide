// Getters and setters in literals, inherited accessors, and the slow paths
// that call them.
var log = '';
var point = {
  x: 1,
  get double() { return this.x * 2; },
  set double(v) { log += 'set' + v + ';'; this.x = v / 2; },
  get onlyGet() { return 'g'; },
  set onlySet(v) { log += 'only' + v + ';'; }
};
print(point.double, point.onlyGet, point.onlySet);
point.double = 10;
print(point.x, point.double, log);
point.onlyGet = 5;
print(point.onlyGet);
function strictWrite(o) { 'use strict'; o.onlyGet = 1; }
try { strictWrite(point); } catch (e) { print(e instanceof TypeError); }
// An inherited accessor runs with the receiver as `this`.
function Thing(n) { this.n = n; }
Thing.prototype = {
  get twice() { return this.n * 2; },
  set label(v) { this.tag = 'L' + v; }
};
var a = new Thing(3), b = new Thing(4);
a.label = 'x';
print(a.twice, b.twice, a.tag, b.tag, 'label' in a);
// Hot accessor sites stay correct across receivers.
var total = 0;
for (var i = 0; i < 50; i++) { var t = new Thing(i); t.label = i; total += t.twice; }
print(total, t.tag);
// A data property after an accessor of the same name replaces it.
var replaced = { get v() { return 'getter'; }, v: 'data' };
var back = { v: 'data', get v() { return 'getter'; } };
print(replaced.v, back.v);
// A getter that throws.
var bad = { get boom() { throw new RangeError('boom'); } };
try { bad.boom; } catch (e) { print(e.name, e.message); }
// Accessors on a prototype chain read through `in` and plain reads.
var base = { get kind() { return 'base:' + this.name; } };
function Child(name) { this.name = name; }
Child.prototype = base;
var c1 = new Child('one'), c2 = new Child('two');
print(c1.kind, c2.kind, 'kind' in c1);
