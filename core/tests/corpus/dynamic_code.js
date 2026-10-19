// Code compiled at run time: functions, generators, regular expressions and
// fresh property names from `eval` and `new Function`, kept and dropped
// while collections run. Every line must print what `dynamic_code.out` holds.
var kept = [];
var total = 0;
for (var i = 0; i < 400; i++) {
  var f = new Function('o', 'return o.k' + i + ' + ' + i);
  var o = {};
  o['k' + i] = i;
  total += f(o);
  if (i % 50 === 0) kept.push(f);
}
print('functions', total, kept.length, kept[3]({ k150: 1 }));
var matched = 0;
for (i = 0; i < 300; i++) {
  var re = eval('/^(a+)b' + (i % 7) + '[c-e]{' + (i % 3) + '}$/');
  var text = 'aab' + (i % 7) + 'cde'.slice(0, i % 3);
  if (re.test(text) && re.exec(text)[1] === 'aa') matched++;
  if (new RegExp('x' + i + '|y').test('y')) matched++;
}
print('regexps', matched, /q(r)s/.exec('qrs')[1]);
var gens = [];
for (i = 0; i < 100; i++) {
  gens.push(eval('(function* g' + i + '(n) { var a = yield n; yield a + ' + i + '; })')(i));
}
var sum = 0;
for (i = 0; i < 100; i++) sum += gens[i].next().value;
for (i = 0; i < 300; i++) eval('(function () { return { p' + i + ': ' + i + ' }; })()');
for (i = 0; i < 100; i++) sum += gens[i].next(1).value;
print('generators', sum, gens[99].next().done);
var names = [];
for (i = 0; i < 200; i++) {
  var made = eval('({ m' + i + ': function n' + i + '() { return "n' + i + '"; } })');
  if (i % 40 === 0) names.push(made['m' + i]);
}
print('names', names.map(function (m) { return m.name + '=' + m(); }).join(','));
print('source', kept[1].toString().split('\n').join(' '));
try {
  eval('\n\nnull.x' + 1);
} catch (error) {
  print('error', error.constructor.name, error.stack.split('\n')[0]);
}
var classes = [];
for (i = 0; i < 20; i++) {
  var C = new Function('return class C' + i + ' { #m() { return ' + i + '; } static #s = ' + i + '; ' +
    'get(o) { return o.#m(); } static s() { return C' + i + '.#s; } }')();
  classes.push(new C());
}
var brands = 0;
for (i = 0; i < 20; i++) {
  try { classes[i].get(classes[(i + 1) % 20]); } catch (error) { brands++; }
}
print('classes', classes[7].get(classes[7]), classes[7].constructor.s(), brands);
