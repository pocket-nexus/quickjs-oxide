// Position case: stack-overflow-like.
function r(n) { if (n === 0) return u.x; return r(n - 1); } var u;
function f() {
  return r(3);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
