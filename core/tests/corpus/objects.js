var o = { x: 1, y: 'two', z: { w: 3 } };
print(o.x, o.y, o.z.w, o.missing);
o.x = 10; o.n = 5; o['computed' + 1] = 'c';
print(o.x, o.n, o.computed1, o['y']);
var key = 'dyn';
var lit = { [key]: 1, a: 2, a: 3, 0: 'zero', 1.5: 'onehalf' };
print(lit.dyn, lit.a, lit[0], lit['0'], lit[1.5], lit['1.5']);
function Point(x, y) { this.x = x; this.y = y; }
Point.prototype.sum = function () { return this.x + this.y; };
var p = new Point(3, 4);
print(p.x, p.y, p.sum(), p instanceof Point, p instanceof Error, 'sum' in p, 'x' in p);
print(p.constructor === Point, Point.prototype.constructor === Point);
function Base() { this.kind = 'base'; }
Base.prototype.hello = function () { return 'hello ' + this.kind; };
function Derived() { this.kind = 'derived'; }
Derived.prototype = new Base();
var d = new Derived();
print(d.hello(), d instanceof Derived, d instanceof Base);
var counter = { n: 0, inc: function () { this.n++; return this; } };
print(counter.inc().inc().inc().n);
var nested = { a: { b: { c: { d: 'deep' } } } };
print(nested.a.b.c.d);
var self = {}; self.me = self; print(self.me.me.me === self);
var holder = { f: function () { return typeof this; } };
print(holder.f(), (0, holder.f)());
var many = {};
for (var i = 0; i < 100; i++) many['k' + i] = i;
var total = 0;
for (i = 0; i < 100; i++) total += many['k' + i];
print(total, many.k99, many.k100);
print(o, p);

// A compound assignment to a computed key converts the key once, after the
// base is checked.
var keyConversions = { n: 0 };
var countedKey = {
  toString: function () {
    keyConversions.n++;
    return "k";
  },
};
var compound = { k: 1 };
compound[countedKey] += 2;
print(compound.k, keyConversions.n);
try {
  var nullBase = null;
  nullBase[countedKey] += 1;
} catch (e) {
  print(e instanceof TypeError, keyConversions.n);
}
