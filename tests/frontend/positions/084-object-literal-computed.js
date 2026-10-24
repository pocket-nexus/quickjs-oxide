// Position case: object-literal-computed.
var u;
function f() {
  return { [u.x]: 1 };
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
