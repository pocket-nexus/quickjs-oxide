// Position case: destructure-array-decl.

function f() {
  var [a] = 1;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
