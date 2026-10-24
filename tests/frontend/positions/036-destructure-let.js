// Position case: destructure-let.

function f() {
  let [a, b] = {};
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
