// Position case: delete-strict.

function f() {
  'use strict'; delete Object.prototype;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
