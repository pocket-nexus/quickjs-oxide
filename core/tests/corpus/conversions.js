var calls = 0;
var obj = { valueOf: function () { calls++; return 41; } };
print(obj + 1, obj * 2, obj > 40, obj == 41, calls);
var str = { toString: function () { return 'S'; } };
print(str + '!', 'x' + str, str == 'S', [str, str] + '');
var both = { valueOf: function () { return 1; }, toString: function () { return 'two'; } };
print(both + 1, '' + both, both * 3);
function Money(v) { this.v = v; }
Money.prototype.valueOf = function () { return this.v; };
Money.prototype.toString = function () { return '$' + this.v; };
var m = new Money(5);
print(m + m, m * 2, m > 4, m < new Money(6));
var o = {};
o[obj] = 'by-value'; o[str] = 'by-string';
print(o['41'], o.S);
var throwing = { valueOf: function () { throw new Error('nope'); } };
try { throwing + 1; } catch (e) { print('caught', e.message); }
print({} + '', [1, [2, 3]] + '', [] == '', [0] == 0, [1] == 1);
var e = new RangeError('r');
print('' + e, e + '!');
