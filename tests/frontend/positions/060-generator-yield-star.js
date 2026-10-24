// Position case: generator-yield-star.
function* gen() { yield* 1; }
function f() {
  gen().next();
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
