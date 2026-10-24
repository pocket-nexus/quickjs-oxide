// Position case: finally-throws.
var u;
function f() {
  try {} finally { u.x; }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
