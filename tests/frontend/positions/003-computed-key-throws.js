// Position case: computed-key-throws.
var o = {}; var k = {toString() { throw new Error('k'); }};
function f() {
  return o[k];
}
function g() {
  return f();
}
try {
  g();
} catch (e) {
  print(e && e.stack !== undefined ? e.stack : String(e));
}
