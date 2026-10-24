// Position case: tagged-member.
var o = {};
function f() {
  return o.t`x${1}`;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
