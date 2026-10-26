// Position case: labeled-loop.
var u;
function f() {
  outer: for (;;) { for (;;) { u.x; break outer; } }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
