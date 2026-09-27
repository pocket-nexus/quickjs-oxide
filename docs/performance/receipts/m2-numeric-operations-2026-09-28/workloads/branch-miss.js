function run(a, i, limit) {
  var count = 0;
  for (var n = 0; n < 2000000; n++) {
    if (a[i] < limit) count++;
  }
  return count;
}
print(run([,], 0, 5));
