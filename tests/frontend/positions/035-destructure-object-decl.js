// Position case: destructure-object-decl.

function f() {
  var {a} = null;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
