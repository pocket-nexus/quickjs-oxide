// Position case: do-while.
var u;
function f() {
  do { u.x; } while (0);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
