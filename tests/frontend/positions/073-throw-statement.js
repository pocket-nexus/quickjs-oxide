// Position case: throw-statement.

function f() {
  throw new Error('t');
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
