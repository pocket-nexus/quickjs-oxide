// Inline caches (phase 3, section 2): monomorphic, polymorphic and
// megamorphic sites, invalidation by prototype changes, global generations
// and constructor sizing. Every line must print the same on both cores.
function getX(o) { return o.x; }
function setX(o, v) { o.x = v; return o; }
var shapes = [];
shapes[0] = { x: 0 };
shapes[1] = { a: 1, x: 1 };
shapes[2] = { a: 1, b: 2, x: 2 };
shapes[3] = { a: 1, b: 2, c: 3, x: 3 };
shapes[4] = { a: 1, b: 2, c: 3, d: 4, x: 4 };
shapes[5] = { a: 1, b: 2, c: 3, d: 4, e: 5, x: 5 };
var sum = 0;
for (var round = 0; round < 3; round++) {
  for (var i = 0; i < 6; i++) sum += getX(shapes[i]);
}
print('poly read', sum);
for (i = 0; i < 6; i++) setX(shapes[i], i * 10);
sum = 0;
for (i = 0; i < 6; i++) sum += getX(shapes[i]);
print('poly write', sum);
// Transitions: fresh objects gain properties through one site.
var last;
for (i = 0; i < 20; i++) { last = setX({}, i); last.y = i * 2; }
print('transition', last.x, last.y);
// A prototype read, then the chain changes under it.
function Animal() {}
Animal.prototype.sound = 'generic';
function Dog() {}
Dog.prototype = new Animal();
var dog = new Dog();
function soundOf(o) { return o.sound; }
var heard = '';
for (i = 0; i < 5; i++) heard += soundOf(dog) + ',';
Dog.prototype.sound = 'woof';
heard += soundOf(dog) + ',';
delete Dog.prototype.sound;
heard += soundOf(dog) + ',';
Animal.prototype.sound = 'changed';
heard += soundOf(dog);
print(heard);
// An own property shadows a cached prototype hit.
dog.sound = 'own';
print(soundOf(dog), soundOf(new Dog()));
// Globals: cached reads and writes, then delete and re-add.
counter = 0;
function bump() { counter = counter + 1; return counter; }
for (i = 0; i < 10; i++) bump();
print('global', counter);
delete globalThis.counter;
try { bump(); } catch (e) { print(e.name); }
counter = 100;
print('readded', bump(), typeof counter);
var declared = 5;
function readDeclared() { return declared; }
for (i = 0; i < 3; i++) readDeclared();
declared = 6;
print('declared', readDeclared(), delete globalThis.declared, readDeclared());
// Constructors: chained initialization grows the receiver.
function init(o) { o.extra1 = 1; o.extra2 = 2; o.extra3 = 3; }
function Wide(n) { this.n = n; init(this); }
var w;
for (i = 0; i < 10; i++) w = new Wide(i);
print('wide', w.n, w.extra1, w.extra3);
Wide.prototype = { tag: 'replaced' };
w = new Wide(1);
print('prototype replaced', w.tag, w instanceof Wide);
// A string receiver's method lookup goes to String.prototype.
var lengths = 0;
var words = ['a', 'bb', 'ccc'];
for (i = 0; i < 3; i++) lengths += words[i].length;
print('strings', lengths);
// Megamorphic: more shapes than a polymorphic row holds.
var mega = 0;
for (round = 0; round < 3; round++) {
  for (i = 0; i < 6; i++) mega += getX(shapes[i]);
  mega += getX({ x: 1000 }) + getX({ q: 0, x: 2000 });
}
print('mega', mega);
