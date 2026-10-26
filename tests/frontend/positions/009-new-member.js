// Position case: new-member.
var o = {};
function f() {
  return new o.C(1);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
