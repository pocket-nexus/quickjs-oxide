function run(a, i, delta) {
  for (var n = 0; n < 2000000; n++) a[i] += delta;
  return a[i];
}
print(run([0], 0, 1));
