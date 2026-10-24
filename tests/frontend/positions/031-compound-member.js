// Position case: compound-member.
var o = {};
function f() {
  o.x.y += 1;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
