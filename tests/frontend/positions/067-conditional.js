// Position case: conditional.
var u;
function f() {
  return u ? 1 : u.x;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
