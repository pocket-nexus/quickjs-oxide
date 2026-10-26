// Position case: computed-undefined.
var u, k = 'x';
function f() {
  return u[k];
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
