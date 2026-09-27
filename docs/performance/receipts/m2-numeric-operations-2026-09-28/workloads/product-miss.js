function run(a, i, scale) {
  var out = 0;
  for (var n = 0; n < 2000000; n++) out = a[i] * scale;
  return out;
}
print(Number.isNaN(run([,], 0, 2)));
