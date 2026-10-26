// Position case: for-of-body-throws.
var u;
function f() {
  for (var x of [1]) { u.x; }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
