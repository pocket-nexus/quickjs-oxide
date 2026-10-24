// Position case: static-block-throws.
var u;
function f() {
  class C { static { u.y; } }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
