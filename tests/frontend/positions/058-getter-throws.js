// Position case: getter-throws.
var o = { get x() { throw new Error('g'); } };
function f() {
  return o.x;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
