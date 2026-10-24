// Position case: neg-symbol.

function f() {
  return -Symbol();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
