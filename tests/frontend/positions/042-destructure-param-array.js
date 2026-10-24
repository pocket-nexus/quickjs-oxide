// Position case: destructure-param-array.
function h([a]) {}
function f() {
  h(1);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
