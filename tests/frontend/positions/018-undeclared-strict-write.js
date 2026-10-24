// Position case: undeclared-strict-write.

function f() {
  'use strict'; missing = 1;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
