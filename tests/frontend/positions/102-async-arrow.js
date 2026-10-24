// Position case: async-arrow.
var u;
function f() {
  (async () => { u.x; })().catch(function (e) { print(e.stack); });
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
