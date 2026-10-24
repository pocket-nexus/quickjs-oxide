// Position case: super-twice.
class B {} class C extends B { constructor() { super(); super(); } }
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
