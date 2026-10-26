// Position case: tdz-closure.

function f() {
  function h() { return y; } h(); let y;
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
