// Position case: try-finally.
var u;
function f() {
  try { u.x; } finally {}
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
