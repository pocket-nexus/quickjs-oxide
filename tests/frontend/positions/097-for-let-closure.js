// Position case: for-let-closure.
var u, fs = [];
function f() {
  for (let i = 0; i < 2; i++) fs.push(() => u.x); fs[0]();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
