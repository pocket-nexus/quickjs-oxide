// Array: Array(n), Array(a, b, ...), new forms, isArray, and push, pop,
// join, slice, concat, indexOf, toString.
var a = new Array(), b = new Array(3), c = Array(1, 2, 3), d = Array('x'), e = new Array(2, 'y');
print(a.length, b.length, b[0], 0 in b, c.join('-'), d.length, d[0], e.join());
try { new Array(-1); } catch (err) { print(err.name); }
try { Array(1.5); } catch (err) { print(err.name); }
print(Array.isArray([]), Array.isArray({}), Array.isArray(arguments = undefined), [] instanceof Array);
var s = [1, 2];
print(s.push(3, 4), s.length, s.pop(), s.pop(), s.length, [].pop(), s);
print([1, [2, [3]], null, undefined, 'x'].join(), [].join(), [1, 2].join(''), [1, 2].join(undefined), [, 1].join());
print([1, 2, 3, 4, 5].slice(1, 3), [1, 2, 3].slice(-2), [1, 2, 3].slice(), [1, 2, 3].slice(5), [1, 2, 3].slice(1, -1), [1, 2, 3].slice(2, 1).length);
var holes = [1, , 3].slice(0);
print(holes.length, 1 in holes);
print([1].concat([2, 3], 4, [[5]]), [].concat(1).length, [1, , 3].concat([4]).length);
print([1, 2, 3, 2].indexOf(2), [1, 2].indexOf(3), [1, 2, 3].indexOf(1, 1), ['a', NaN].indexOf(NaN), [1, 2, 3].indexOf(3, -1), [0].indexOf(-0));
print([1, 2, 3].toString(), String([4, [5, 6]]), [] + [], [null] + '', [1, 2] + 1);
var obj = { length: 2, 0: 'a', 1: 'b' };
print(Array.prototype.join.call(obj, '+'), Array.prototype.slice.call(obj, 1), Array.prototype.push.call(obj, 'c'), obj.length, obj[2]);
var big = new Array(10000); big[9999] = 'last';
print(big.length, big[9998], big[9999]);
var grow = []; for (var i = 0; i < 2000; i++) grow[i] = i; print(grow.length, grow[1999]);
