// Position case: string-method.

function f() {
  return ''.repeat(-1);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
