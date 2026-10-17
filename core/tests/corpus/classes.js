// Classes: methods, accessors, statics, fields, private names, super.
class Point {
  #x;
  #y = 0;
  static count = 0;
  static #registry = [];
  static { Point.origin = new Point(0, 0); }
  constructor(x, y) {
    this.#x = x;
    this.#y = y;
    Point.count++;
    Point.#registry.push(this);
  }
  get x() { return this.#x; }
  set x(value) { this.#x = value; }
  get #sum() { return this.#x + this.#y; }
  set #scaled(factor) { this.#x *= factor; this.#y *= factor; }
  #describe() { return "(" + this.#x + ", " + this.#y + ")"; }
  scale(factor) { this.#scaled = factor; return this; }
  toString() { return this.#describe() + " sum " + this.#sum; }
  static has(o) { return #x in o; }
  static registered() { return Point.#registry.length; }
  ["comp" + "uted"]() { return "computed"; }
  static ["s" + 1]() { return "static computed"; }
}
var p = new Point(1, 2);
print(String(p), p.x, Point.count, Point.registered());
p.x = 5;
print(String(p.scale(2)), Point.has(p), Point.has({}), p.computed(), Point.s1());
print(typeof Point, Object.getOwnPropertyNames(Point.prototype).join());
try { Point(); } catch (e) { print(e.constructor.name); }

class Animal {
  constructor(name) { this.name = name; }
  speak() { return this.name + " makes a sound"; }
  get kind() { return "animal"; }
  static create(name) { return new this(name); }
}
class Dog extends Animal {
  constructor(name) {
    super(name);
    this.legs = 4;
    var self = () => this;
    print("same this", self() === this);
  }
  speak() { return super.speak() + " (woof)"; }
  get kind() { return "dog, an " + super.kind; }
  set nickname(value) { super.nickname = value; }
}
var d = Dog.create("Rex");
print(d.speak(), d.kind, d.legs, d instanceof Animal);
d.nickname = "R";
print(d.nickname, Object.keys(d).join());

class Lazy extends Animal {
  constructor() {
    var init = () => super("lazy");
    init();
  }
}
print(new Lazy().speak());

class Broken extends Animal {
  constructor() { return 1; }
}
try { new Broken(); } catch (e) { print(e.constructor.name); }

class Replaced extends Animal {
  constructor() { super("x"); return { replaced: true }; }
}
print(new Replaced().replaced);

var Expr = class Named {
  who() { return Named.name; }
};
print(new Expr().who(), Expr.name);

class Counter {
  static #count = 0;
  static increment() { return ++Counter.#count; }
  #secret = 42;
  static read(o) { try { return o.#secret; } catch (e) { return e.constructor.name; } }
}
Counter.increment();
print(Counter.increment(), Counter.read(new Counter()), Counter.read({}));

var obj = {
  __proto__: { greet() { return "proto"; } },
  greet() { return "own > " + super.greet(); },
};
print(obj.greet());

class Defaulted extends Animal {}
print(new Defaulted("default").speak());
class Branded {
  #method() { return 1; }
  static check(o) { return #method in o; }
}
print(Branded.check(new Branded()), Branded.check({}));

class Deleter {
  m() { try { delete super.x; } catch (e) { return e.constructor.name; } }
}
print(new Deleter().m());
