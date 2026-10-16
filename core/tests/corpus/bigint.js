var a = 12345678901234567890n, b = 98765432109876543210n;
print(a + b, b - a, a * b, b / a, b % a, -a, a ** 3n);
print(1n << 64n, (1n << 64n) - 1n, -5n >> 1n, 5n & 3n, -5n | 3n, 5n ^ -3n, ~5n);
print(a < b, a > b, a == 12345678901234567890n, a === b, 1n == 1, 2n > 1, 1n < 1.5, 0n == '');
print(typeof a, 10n / 3n, -10n / 3n, -10n % 3n, 0n ? 't' : 'f', 5n ? 't' : 'f');
try { 1n + 1; } catch (e) { print(e instanceof TypeError); }
try { 1n / 0n; } catch (e) { print(e instanceof RangeError); }
var x = (1n << 256n) - 1n, base = x;
for (var i = 0; i < 10; i++) { x = ((x ^ 5n) + 1n) - 1n; x = x ^ 5n; }
print(x === base, x);
var c = 0n; c++; c++; c--; print(c, 'v' + 7n);
