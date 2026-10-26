// Position case: super-call-args.
var u; class B {} class C extends B { constructor() { super(u.x); } }
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
