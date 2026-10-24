// Position case: closure-capture.
var u; function mk() { var v = u; return function () { return v.x; }; }
function f() {
  return mk()();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
