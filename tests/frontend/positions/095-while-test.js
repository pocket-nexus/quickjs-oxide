// Position case: while-test.
var u;
function f() {
  while (u.x) {}
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
