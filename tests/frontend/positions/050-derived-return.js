// Position case: derived-return.
class B {} class C extends B { constructor() { super(); return 1; } }
function f() {
  new C();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
