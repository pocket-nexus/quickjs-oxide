// Position case: derived-this.
class B {} class C extends B { constructor() { this.x = 1; super(); } }
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
