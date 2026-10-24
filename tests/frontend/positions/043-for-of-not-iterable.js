// Position case: for-of-not-iterable.

function f() {
  for (var x of 1) {}
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
