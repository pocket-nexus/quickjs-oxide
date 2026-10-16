// String: String(v), fromCharCode and the prototype methods.
print(String(), String(12), String(null), String(undefined), String(true), String([1, 2]), typeof String('x'));
print(String.fromCharCode(72, 105), String.fromCharCode(), String.fromCharCode(65.7, 65536 + 66, -1).length, String.fromCharCode(0x263A));
var t = 'hello world';
print(t.charAt(0), t.charAt(-1), t.charAt(99), t.charAt(1.9), t.charAt(), t.charCodeAt(1), t.charCodeAt(99), t.charCodeAt(-1));
print(t.indexOf('o'), t.indexOf('o', 5), t.indexOf('z'), t.indexOf(''), t.indexOf('', 99), t.indexOf('o', -5));
print(t.lastIndexOf('o'), t.lastIndexOf('o', 5), t.lastIndexOf('z'), t.lastIndexOf(''), t.lastIndexOf('h', -1), t.lastIndexOf('d', NaN));
print(t.slice(1, 4), t.slice(-5), t.slice(4, 1), t.slice(2), t.slice(-3, -1), t.slice(0, 1.9));
print(t.substring(1, 4), t.substring(4, 1), t.substring(-3), t.substring(5, NaN), t.substring(3, 99));
print(t.substr(1, 3), t.substr(-5, 2), t.substr(2), t.substr(1, -1), t.substr(99, 1).length);
print('a'.concat('b', 1, null), ''.concat(), 'x'.concat([1, 2]));
print('a,b,,c'.split(','), 'abc'.split(''), 'abc'.split(), ''.split(','), ''.split(''), 'a,b,c'.split(',', 2), 'abc'.split('', 0).length, 'aXbXc'.split('X'));
print('x-y-z'.replace('-', '+'), 'abc'.replace('z', '!'), 'abc'.replace('b', '[$&$&]'), 'abc'.replace('b', "$`|$'"), 'abc'.replace('b', '$$'), 'abc'.replace('b', '$1'), 'abc'.replace('', '_'));
print('MiXeD 123'.toLowerCase(), 'MiXeD 123'.toUpperCase(), 'ÄÖÜ'.toLowerCase(), 'straße'.toUpperCase(), 'ǅ'.toLowerCase());
print('abc'.toString(), 'abc'.valueOf(), 'abc'.length, 'abc'[1], 'abc'['length']);
try { String.prototype.valueOf.call(1); } catch (e) { print(e.name); }
try { String.prototype.charAt.call(null, 0); } catch (e) { print(e.name); }
var built = ''; for (var i = 0; i < 100; i++) built += String.fromCharCode(97 + i % 26);
print(built.length, built.charCodeAt(99), built.slice(-3));
print('Ünïcödé'.indexOf('c'), 'Ünïcödé'.slice(1, 3), 'Ünïcödé'.charCodeAt(0));
