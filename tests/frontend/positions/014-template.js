// Position case: template.
var u;
function f() {
  return `a${u.x}b`;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
