// Position case: compound-computed.
var u, k = 'k';
function f() {
  u[k] += 1;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
