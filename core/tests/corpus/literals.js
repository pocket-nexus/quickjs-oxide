// Literal forms: accessors, __proto__, computed methods, templates.
var counter = 0;
var o = {
  get value() { return counter; },
  set value(v) { counter = v * 2; },
  ["key" + 1]() { return "computed method"; },
  get ["dyn" + "amic"]() { return "computed getter"; },
  set ["dyn" + "amic"](v) { counter = -v; },
  __proto__: { inherited: "yes" },
  async method() {},
  *gen() {},
};
o.value = 4;
print(o.value, o.key1(), o.dynamic, o.inherited, typeof o.method, typeof o.gen);
o.dynamic = 3;
print(counter);

const LIMIT = 3;
let mutable = 1;
try { LIMIT = 4; } catch (e) { print(e.constructor.name); }
print(LIMIT, mutable);

var name = "world";
print(`hello ${name} ${1 + 2}`, `multi
line`);
function tag(strings, ...values) {
  return strings.raw.join("|") + " / " + values.join(",") + " " + Object.isFrozen(strings);
}
print(tag`a${1}b${2}c\n`);
function same(s) { return s; }
var first = [];
for (var i = 0; i < 2; i++) first.push(same`x`);
print(first[0] === first[1]);

var { a, ...restProps } = { a: 1, b: 2, c: 3 };
print(a, Object.keys(restProps).join());
for (var k in "ab") print("key", k);
var primitive = 5;
var { toFixed } = primitive;
print(typeof toFixed);
