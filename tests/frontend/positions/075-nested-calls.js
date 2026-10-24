// Position case: nested-calls.
function a() { return b(); } function b() { return c(); } function c() { return u.x; } var u;
function f() {
  return a();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
