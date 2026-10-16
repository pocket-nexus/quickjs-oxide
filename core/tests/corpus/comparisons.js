print(1 < 2, 2 < 1, 1 <= 1, 2 >= 3, 1 > NaN, NaN <= NaN, !(NaN < 1));
print('a' < 'b', 'ab' < 'a', 'B' < 'a', '10' < '9', '10' < 9, 10 < '9');
print(null == undefined, null === undefined, null == 0, undefined == 0, 0 == '', '1' == 1);
print(true == 1, false == '0', 'true' == true, NaN == NaN, -0 === 0, 1 === 1.0);
print(null >= 0, null > 0, undefined >= 0, '' == 0, ' \n' == 0);
var o = {}; var p = o;
print(o == p, o === p, o == {}, o != null, o !== undefined);
function cmp(a, b) { if (a < b) return -1; if (a > b) return 1; return 0; }
print(cmp(1, 2), cmp('b', 'a'), cmp(3, 3), cmp(NaN, 1));
function negated(a, b) { var r = ''; if (!(a < b)) r += 'nlt'; if (!(a <= b)) r += 'nle'; if (!(a > b)) r += 'ngt'; if (!(a >= b)) r += 'nge'; return r; }
print(negated(1, NaN), negated(2, 1), negated(1, 1));
var n = 0;
for (var i = 10; i > -5; i--) if (i >= 0 && i <= 3) n++;
print(n);
print(1 == 1 ? 'yes' : 'no', 0 ? 'yes' : 'no', '' ? 1 : 2, 'x' ? 1 : 2);
var big = 100000; print(big < 32767, big > -32768, 40000 > 32767);
