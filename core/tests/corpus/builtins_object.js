// Object: Object(), new Object(), Object.prototype.toString, valueOf,
// hasOwnProperty, constructor.
var o = Object(), p = new Object(), same = { a: 1 };
print(typeof o, typeof p, Object(same) === same, new Object(same) === same, Object(null) !== null);
print(Object.prototype.toString.call([]), Object.prototype.toString.call({}),
      Object.prototype.toString.call(function () {}), Object.prototype.toString.call(null),
      Object.prototype.toString.call(undefined), Object.prototype.toString.call(1),
      Object.prototype.toString.call('s'), Object.prototype.toString.call(true),
      Object.prototype.toString.call(new Error('e')), Object.prototype.toString.call(/x/));
(function () { print(Object.prototype.toString.call(arguments)); })();
print(String({}), '' + { toString: function () { return 'custom'; } }, { valueOf: function () { return 4; } } * 2);
print(same.valueOf() === same, same.hasOwnProperty('a'), same.hasOwnProperty('b'), same.hasOwnProperty('toString'));
print([1, 2].hasOwnProperty(0), [1, 2].hasOwnProperty('length'), 'abc'.hasOwnProperty(1), 'abc'.hasOwnProperty('length'));
function F() { this.own = 1; } F.prototype.inherited = 2;
var f = new F();
print(f.hasOwnProperty('own'), f.hasOwnProperty('inherited'), f.constructor === F, ({}).constructor === Object);
print(Object.prototype.hasOwnProperty.call({ k: 1 }, 'k'), typeof Object.prototype.toString);
try { Object.prototype.valueOf.call(null); } catch (e) { print(e.name); }
