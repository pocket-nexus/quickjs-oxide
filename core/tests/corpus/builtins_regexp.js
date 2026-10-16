// RegExp: the constructor, exec, test, toString, the flag accessors, and
// the regular-expression paths of split, replace and match.
var r = /(\d+)-(\d+)/;
var m = r.exec('call 555-1234 now');
print(m.length, m[0], m[1], m[2], m.index, m.input, m.groups, r.lastIndex);
print(r.exec('none'), r.test('1-2'), r.test('x'), String(r), r.source, r.flags, r.global, r.ignoreCase, r.multiline);
var g = /a(b)?/g;
print(g.exec('ab a ab').index, g.lastIndex, g.exec('ab a ab').index, g.lastIndex, g.exec('ab a ab').index, g.lastIndex, g.exec('ab a ab'), g.lastIndex);
var m2 = /a(b)?/.exec('a'); print(m2[1], m2.length, 1 in m2);
g.lastIndex = 99; print(g.test('ab'), g.lastIndex);
var y = /a/y; y.lastIndex = 1; print(y.test('ba'), y.lastIndex, y.test('ba'), y.lastIndex);
print(new RegExp('a+', 'gi').flags, RegExp('x').source, new RegExp('/', '').source, String(new RegExp('')), RegExp(/q/g) === RegExp(/q/g));
var same = /z/; print(RegExp(same) === same, new RegExp(same) === same, new RegExp(same, 'g').flags, new RegExp('a', 'mig').flags);
try { new RegExp('('); } catch (e) { print(e.name); }
try { new RegExp('a', 'gg'); } catch (e) { print(e.name); }
print('a1b22c333'.replace(/\d+/, '#'), 'a1b22c333'.replace(/\d+/g, '#'), 'aaa'.replace(/a/g, '$&$&'), 'John Smith'.replace(/(\w+)\s(\w+)/, '$2, $1'));
print('abc'.replace(/b/, "[$`|$']"), 'abc'.replace(/(x)?b/, '[$1]'), 'abc'.replace(/b/, '$$'), 'abc'.replace(/b/, '$0$3'), 'aaa'.replace(/x*/g, '-'));
print('2026-10-16'.replace(/(?<y>\d+)-(?<m>\d+)-(?<d>\d+)/, '$<d>/$<m>/$<y>'), /(?<word>\w+)/.exec('hi there').groups.word, 'ab'.replace(/(?<x>a)/, '[$<nope>]'));
print('a, b ,c'.split(/\s*,\s*/), 'a1b2c'.split(/(\d)/), 'abc'.split(/(?:)/), ''.split(/x/).length, ''.split(/(?:)/).length, 'a,b,c'.split(/,/, 2), 'ab'.split(/(x)?/));
print('The Quick Brown'.match(/[A-Z]/g), 'xyz'.match(/a/g), 'xyz'.match(/y/).index, 'a.b'.match('.').length, 'aaa'.match(/a*?/g).length, 'abc'.match().index);
print(/[a-z]+/i.test('HELLO'), /^b/m.test('a\nb'), /\u{1F600}/u.test('\u{1F600}'), /./su.exec('\u{1F600}')[0].length, /ß/i.test('ẞ'));
var text = 'one two three four';
var words = 0, re = /\w+/g; while (re.exec(text)) words++;
print(words, re.lastIndex);
var big = ''; for (var i = 0; i < 200; i++) big += 'x' + i + ' ';
print(big.replace(/x(\d+)/g, '$1').length, big.split(/\s+/).length, big.match(/\d+/g).length);
print(/a/ instanceof RegExp, Object.prototype.toString.call(/a/), typeof RegExp.prototype.exec, RegExp.prototype.source, RegExp.prototype.global);
try { RegExp.prototype.exec.call({}, 'x'); } catch (e) { print(e.name); }
print('ÀÉÎ'.toLowerCase(), 'ΣΑΣ'.toLowerCase(), 'ﬀ'.toUpperCase(), 'İ'.toLowerCase().length);
