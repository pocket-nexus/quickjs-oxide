// Property keys made from strings at run time, held across JavaScript (proxy
// traps, a constructor's prototype read) while collections run. Every line
// must print what `weak_atoms.out` holds.
function churn() {
  var made = [];
  for (var i = 0; i < 200; i++) made.push({ ['c' + i]: 'v' + i });
  return made.length;
}
// Names computed at run time and never spelled in the source, so no unit
// holds their atoms.
function name(prefix, n) { return prefix + n; }
var keys = Reflect.ownKeys(new Proxy({}, {
  ownKeys: function () { return [name('x', 1), name('y', 2)]; }
}));
churn();
print('ownKeys', keys.join(), keys[0] === name('x', 1));
// The target lacks the keys the traps report, so only the enumeration
// holds them.
var walked = new Proxy({}, {
  ownKeys: function () { return [name('a', 1), name('b', 2)]; },
  getOwnPropertyDescriptor: function (t, k) {
    churn();
    return { value: k, enumerable: true, configurable: true };
  }
});
var seen = [];
for (var k in walked) {
  churn();
  seen.push(k);
}
print('for-in', seen.join());
var fixed = {};
Object.defineProperty(fixed, name('q', 1), { value: 5, configurable: false, writable: false });
var trapped = new Proxy(fixed, {
  get: function (t, k) { churn(); return k in t ? t[k] : k; },
  has: function (t, k) { churn(); return k in t; },
  defineProperty: function (t, k, d) { churn(); return Reflect.defineProperty(t, k, d); },
  deleteProperty: function (t, k) { churn(); return delete t[k]; }
});
print('get', trapped[name('q', 1)], trapped[name('s', 1)], name('q', 1) in trapped);
Object.defineProperty(trapped, name('r', 1), { value: 7, configurable: true });
print('define', fixed[name('r', 1)], delete trapped[name('r', 1)], name('r', 1) in fixed);
var re = new RegExp(name('ab+', 'c'), name('g', 'i'));
var newTarget = new Proxy(function () {}, {
  get: function (t, k) {
    if (k === 'prototype') {
      re.compile(name('z', 9), 'm');
      churn();
      return RegExp.prototype;
    }
    return t[k];
  }
});
var made = Reflect.construct(RegExp, [re], newTarget);
churn();
print('regexp', made.source, made.flags, re.source, re.flags);
var refusing = new Proxy({}, {
  set: function () { churn(); return false; },
  deleteProperty: function () { churn(); return false; }
});
var messages = [];
(function () {
  'use strict';
  try { refusing[name('w', 1)] = 1; } catch (e) { messages.push(e.constructor.name); }
  try { delete refusing[name('w', 2)]; } catch (e) { messages.push(e.constructor.name); }
})();
print('refused', messages.join());
