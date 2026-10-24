// Position case: for-update.
var u;
function f() {
  for (var i = 0; i < 1; u.x) {}
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
