// Position case: super-property.
class B { m() { return this.u.x; } } class C extends B { m() { return super.m(); } }
function f() {
  return new C().m();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
