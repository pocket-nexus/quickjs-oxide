// Position case: spread-call.
var a = [1];
function f() {
  return u(...a);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
