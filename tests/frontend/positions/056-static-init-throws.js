// Position case: static-init-throws.
var u;
function f() {
  class C { static x = u.y; }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
