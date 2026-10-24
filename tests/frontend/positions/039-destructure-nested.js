// Position case: destructure-nested.

function f() {
  var {a: {b}} = {};
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
