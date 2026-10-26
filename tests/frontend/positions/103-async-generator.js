// Position case: async-generator.
var u; async function* h() { yield 1; u.x; }
function f() {
  var it = h(); it.next(); it.next().catch(function (e) { print(e.stack); });
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
