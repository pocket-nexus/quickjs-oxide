// Position case: default-param.
var u; function h(a = u.x) {}
function f() {
  h();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
