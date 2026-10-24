// Position case: async-await.
var u; async function h() { await 0; u.x; }
function f() {
  h().catch(function (e) { print(e.stack); });
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
