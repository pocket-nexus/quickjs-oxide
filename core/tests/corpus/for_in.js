// for-in.
function keys(o) { var s = ''; for (var k in o) s += k + ','; return s; }
print(keys({ a: 1, b: 2, c: 3 }));
print(keys([10, 20, 30]), keys([]), keys(null), keys(undefined), keys(5), keys('ab'));
var sparse = [1, , 3]; sparse.x = 'named';
print(keys(sparse));
function Animal() { this.own = 1; }
Animal.prototype.inherited = 2;
Animal.prototype.own = 'shadowed';
print(keys(new Animal()));
var mixed = { b: 1, 2: 'two', a: 2, 1: 'one' };
print(keys(mixed));
var deleting = { a: 1, b: 2, c: 3 };
var seen = '';
for (var k in deleting) { seen += k; delete deleting.b; }
print(seen);
var adding = { a: 1 };
seen = '';
for (k in adding) { seen += k; adding.z = 1; }
print(seen);
var dict = { a: 1, b: 2, c: 3 }; delete dict.a; dict.d = 4;
print(keys(dict));
function F() {} F.prototype = { p: 1 };
var f = new F(); f.q = 2;
print(keys(f));
print(keys(function () {}));
var nested = '';
for (var i in { x: 1, y: 2 }) for (var j in [7, 8]) nested += i + j;
print(nested);
function argKeys() { return keys(arguments); }
print(argKeys('a', 'b'));
var count = 0;
for (var key in { a: 1, b: 2, c: 3 }) { if (key === 'b') break; count++; }
print(count);
