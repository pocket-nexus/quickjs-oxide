// Position case: array-method-callback.
var u;
function f() {
  return [1].map(function () { return u.x; });
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
