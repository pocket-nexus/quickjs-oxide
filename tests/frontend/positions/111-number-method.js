// Position case: number-method.

function f() {
  return (1).toFixed(200);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
