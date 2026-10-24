// Position case: call-in-args.
var u; function h() {}
function f() {
  return h(u.x);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
