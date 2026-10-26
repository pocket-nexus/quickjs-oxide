// Position case: private-missing.
class C { #x; static get(o) { return o.#x; } }
function f() {
  C.get({});
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
