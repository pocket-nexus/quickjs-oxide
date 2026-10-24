// Position case: new-undefined.
var U;
function f() {
  return new U();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
