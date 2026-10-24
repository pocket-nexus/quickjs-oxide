// Position case: object-literal-value.
var u;
function f() {
  return { a: u.x };
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
