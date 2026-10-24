// Position case: spread-not-iterable.
function h() {}
function f() {
  return h(...1);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
