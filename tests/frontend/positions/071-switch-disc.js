// Position case: switch-disc.
var u;
function f() {
  switch (u.x) { case 1: }
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
