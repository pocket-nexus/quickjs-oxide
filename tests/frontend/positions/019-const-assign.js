// Position case: const-assign.
const c = 1;
function f() {
  c = 2;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
