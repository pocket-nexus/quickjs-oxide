// Position case: extends-non-constructor.

function f() {
  class C extends 1 {}
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
