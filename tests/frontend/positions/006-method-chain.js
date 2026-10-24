// Position case: method-chain.
var o = {m() { return 1; }};
function f() {
  return o.m()();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
