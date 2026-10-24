// Position case: destructure-object-assign.
var a;
function f() {
  ({a} = null);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
