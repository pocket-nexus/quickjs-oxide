// Position case: call-args-multiline.
var o = {};
function f() {
  return o.m(
  1,
  2
);
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
