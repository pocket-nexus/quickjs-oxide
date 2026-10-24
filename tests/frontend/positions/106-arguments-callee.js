// Position case: arguments-callee.

function f() {
  'use strict'; return arguments.callee;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
