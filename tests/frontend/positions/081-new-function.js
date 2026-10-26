// Position case: new-function.

function f() {
  return new Function('return u.x')();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
