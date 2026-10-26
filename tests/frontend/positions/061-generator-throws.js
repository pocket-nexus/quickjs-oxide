// Position case: generator-throws.
var u; function* gen() { yield 1; u.x; }
function f() {
  var it = gen(); it.next(); it.next();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
