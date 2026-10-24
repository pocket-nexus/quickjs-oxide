// Position case: spread-method.
var o = {}, a = [1];
function f() {
  return o.m(...a);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
