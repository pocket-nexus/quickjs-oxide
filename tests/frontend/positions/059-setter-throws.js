// Position case: setter-throws.
var o = { set x(v) { throw new Error('s'); } };
function f() {
  o.x = 1;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
