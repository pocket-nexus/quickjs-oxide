function run() {
  var count = 0;
  for (var n = 0; n < 2000000; n++) count++;
  return count;
}
print(run());
