// Position case: destructure-default-throws.

function f() {
  var [a = u.x] = [];
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
