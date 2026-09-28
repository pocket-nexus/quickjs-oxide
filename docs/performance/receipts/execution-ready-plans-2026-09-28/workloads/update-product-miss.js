function run(x, s, dt, i) {
  for (var n = 0; n < 2000000; n++) x[i] += dt * s[i];
  return x[i];
}
var s = [];
Object.defineProperty(s, '0', {get() { return 1; }});
print(run([0], s, 1, 0));
