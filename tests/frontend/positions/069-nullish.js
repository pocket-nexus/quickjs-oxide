// Position case: nullish.
var u;
function f() {
  return u ?? u.x;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
