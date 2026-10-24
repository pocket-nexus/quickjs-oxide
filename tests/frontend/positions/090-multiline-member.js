// Position case: multiline-member.
var u;
function f() {
  return u
  .x
  .y;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
