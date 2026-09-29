// Callback-capable native calls reenter a native method before resuming.
function run(count) {
  var map = new Map(), key = {}, calls = 0, sum = 0, minimum = Math.min;
  map.set(key, 42);
  var value = {valueOf: function() { calls++; return map.get(key); }};
  for (var i = 0; i < count; i++) sum += minimum(value, 50);
  return sum + ':' + calls;
}
print(run(30000));
