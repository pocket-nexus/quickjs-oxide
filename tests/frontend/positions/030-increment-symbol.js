// Position case: increment-symbol.
var s = Symbol();
function f() {
  s++;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
