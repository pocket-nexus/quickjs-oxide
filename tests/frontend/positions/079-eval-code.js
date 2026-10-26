// Position case: eval-code.
var u;
function f() {
  return eval('u.x');
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
