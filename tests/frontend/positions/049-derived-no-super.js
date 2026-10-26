// Position case: derived-no-super.
class B {} class C extends B { constructor() {} }
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
