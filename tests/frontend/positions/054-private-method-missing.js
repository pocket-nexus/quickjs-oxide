// Position case: private-method-missing.
class C { #m() {} static call(o) { return o.#m(); } }
function f() {
  C.call({});
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
