// Position case: multiline-call.
var u;
function f() {
  return u
  (
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
