// Position case: switch-case.
var u;
function f() {
  switch (1) { case u.x: }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
