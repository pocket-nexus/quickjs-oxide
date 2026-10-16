// globalThis members (section 6.2): parseInt, parseFloat, isNaN, isFinite.
print(parseInt('42'), parseInt('  -0x1F'), parseInt('ff', 16), parseInt('0x10', 16), parseInt('z', 36));
print(parseInt('12px'), parseInt(''), parseInt('abc'), parseInt('10', 2), parseInt('10', 37), parseInt('10', 1));
print(parseInt('123456789012345678901234567890'), parseInt('-0'), 1 / parseInt('-0'), parseInt(4.7), parseInt('0.00001'));
print(parseInt('11', 0), parseInt('11', '8'), parseInt('  +7 '), parseInt('08'), parseInt(null, 36));
print(parseFloat('3.14abc'), parseFloat('  -.5'), parseFloat('1e3'), parseFloat('1e'), parseFloat('Infinityx'), parseFloat('x'));
print(parseFloat('0.1e-2'), parseFloat('-0'), 1 / parseFloat('-0'), parseFloat('123456789012345678901234567890'));
print(isNaN(NaN), isNaN('x'), isNaN('12'), isNaN(undefined), isNaN(null), isNaN({}));
print(isFinite(1), isFinite(Infinity), isFinite('7'), isFinite(NaN), isFinite('x'));
print(typeof globalThis, globalThis.parseInt === parseInt, NaN !== NaN, Infinity > 1e308);
