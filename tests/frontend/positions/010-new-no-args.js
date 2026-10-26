// Position case: new-no-args.
var o = {};
function f() {
  return new o.C;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
