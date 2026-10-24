// Position case: destructure-assign.
var a;
function f() {
  [a] = 1;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
