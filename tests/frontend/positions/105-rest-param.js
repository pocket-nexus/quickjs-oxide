// Position case: rest-param.
var u; function h(...r) { return u.x; }
function f() {
  h(1);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
