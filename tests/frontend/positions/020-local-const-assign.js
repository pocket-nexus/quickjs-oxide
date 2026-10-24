// Position case: local-const-assign.

function f() {
  const c = 1; c = 2;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
