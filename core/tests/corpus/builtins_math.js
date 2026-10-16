// Math: the functions and constants of section 6.2, with NaN and ±0.
print(Math.abs(-3), Math.abs(-0), 1 / Math.abs(-0), Math.abs('x'), Math.abs(-2147483648));
print(Math.floor(1.7), Math.floor(-1.2), 1 / Math.floor(-0), Math.ceil(1.2), Math.ceil(-1.7), 1 / Math.ceil(-0.5));
print(Math.round(2.5), Math.round(-2.5), Math.round(0.49999999999999994), 1 / Math.round(-0.4), Math.round(1e20), Math.round(-0.5), Math.round(0.5), Math.round(NaN));
print(Math.min(), Math.max(), Math.min(1, 2), Math.max(1, 'x'), Math.min(0, -0), 1 / Math.min(0, -0), 1 / Math.max(-0, 0), Math.max(-1, -5, 3));
print(Math.pow(2, 10), Math.pow(2, -1), Math.pow(NaN, 0), Math.pow(1, Infinity), Math.pow(-8, 1 / 3), Math.pow(-2, 3));
print(Math.sqrt(16), Math.sqrt(-1), Math.sqrt(2), Math.log(Math.E), Math.log(0), Math.exp(1), Math.exp(0));
print(Math.sin(0), Math.cos(0), Math.tan(0), Math.asin(1), Math.acos(1), Math.atan(1), Math.sin(Math.PI / 2));
print(Math.E, Math.LN2, Math.LN10, Math.LOG2E, Math.LOG10E, Math.PI, Math.SQRT1_2, Math.SQRT2);
var r = Math.random(); print(r >= 0 && r < 1, typeof r);
var sum = 0; for (var i = 0; i < 1000; i++) { var x = Math.random(); if (x < 0 || x >= 1) sum++; } print(sum);
print(Math.floor(7 / 2), Math.max.apply(null, [3, 9, 1]), Math.min.call(null, 4, 2));
