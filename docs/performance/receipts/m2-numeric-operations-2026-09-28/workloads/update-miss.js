function run(a, i, delta) {
  for (var n = 0; n < 2000000; n++) a[i] += delta;
  return a[i];
}
var a = [0];
Object.defineProperty(a, '0', {value: 0, writable: false});
try { print(run(a, 0, 1)); } catch (e) { print(e.name); }
