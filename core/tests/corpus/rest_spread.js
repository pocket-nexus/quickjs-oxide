// Rest parameters and spread of arrays and `arguments` objects.
function rest(first, ...others) { return first + ':' + others.length + ':' + others; }
print(rest(), rest(1), rest(1, 2, 3));
var collect = (...all) => all.length;
print(collect(), collect(1, 2), collect.apply(null, [1, 2, 3]));
function add(a, b, c) { return a + b + c; }
var nums = [1, 2, 3];
print(add(...nums), add(0, ...[5, 6]), add(...nums, 10), add(...[]));
function forward() { return add(...arguments); }
print(forward(1, 2, 3), forward('a', 'b', 'c'));
function Point(x, y) { this.x = x; this.y = y; }
var p = new Point(...[3, 4]);
print(p.x, p.y, p instanceof Point);
var joined = [0, ...nums, ...[4, 5], 6];
print(joined.length, joined[0], joined[3], joined[6]);
var holes = [, 1];
var copy = [...holes];
print(copy.length, copy[0], 0 in copy);
var obj = { f: function (...xs) { return this.tag + xs.length; }, tag: 't' };
print(obj.f(...nums));
function restArgs(a, ...r) { return arguments.length + ' ' + r.length; }
print(restArgs(1, 2, 3));
