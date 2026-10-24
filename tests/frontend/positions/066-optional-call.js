// Position case: optional-call.
var o = {a: {}};
function f() {
  return o?.a.b();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
