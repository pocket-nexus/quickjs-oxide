// Position case: optional-chain.
var o = {};
function f() {
  return o?.a.b;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
