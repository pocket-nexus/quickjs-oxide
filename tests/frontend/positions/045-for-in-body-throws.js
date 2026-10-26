// Position case: for-in-body-throws.
var u;
function f() {
  for (var k in {a: 1}) { u.x; }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
