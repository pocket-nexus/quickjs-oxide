function run(x, s, dt, i) {
  for (var n = 0; n < 2000000; n++) x[i] += dt * s[i];
  return x[i];
}
print(run([0], [1], 1, 0));
