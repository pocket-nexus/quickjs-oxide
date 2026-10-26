// Position case: catch-throws.
var u;
function f() {
  try { throw 1; } catch (e) { u.x; }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
