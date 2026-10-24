// Position case: multiline-binary.

function f() {
  return 1n
  +
  1;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
