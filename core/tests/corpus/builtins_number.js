// Number: Number(v), toString(radix), valueOf.
print(Number(), Number('12'), Number(' 0x1F '), Number('x'), Number(null), Number(undefined), Number(true), Number([5]), Number('1e3'));
print((255).toString(16), (255).toString(2), (-255).toString(36), (0.5).toString(2), (3.75).toString(8), (1e21).toString(36));
print((0.1).toString(3), (1 / 3).toString(5), (-0).toString(16), NaN.toString(2), (Infinity).toString(7), (123.456).toString());
print((2 ** 53).toString(16), (2 ** 60 + 1).toString(10), (1e-7).toString(), (0.000001).toString(), (1.5e300).toString(36).length);
try { (1).toString(1); } catch (e) { print(e.name); }
try { (1).toString(37); } catch (e) { print(e.name); }
print((5).valueOf(), (2.5).toString(10), 12..toString(), (0x7fffffff).toString(16), (0xffffffff).toString(16), (-1 >>> 0).toString(2).length);
try { Number.prototype.valueOf.call('x'); } catch (e) { print(e.name); }
print(String(0.1 + 0.2), String(2 ** -1017), String(192181864906432.125), String(1e21), String(123456789012345680000), String(-1e-7));
var hex = ''; for (var i = 0; i < 20; i++) hex += (i * 7919).toString(16) + ','; print(hex);
