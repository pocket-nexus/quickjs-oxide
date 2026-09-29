// Direct and named native calls with primitive arguments, including zero arity.
function run(count) {
  var minimum = Math.min, maximum = Math.max, sum = 0;
  if (maximum() !== -Infinity) throw new Error('zero arity');
  for (var i = 0; i < count; i++) {
    var value = i % 100;
    sum += minimum(value, 50) + maximum(value, 50) + Number.isFinite(value);
  }
  return sum;
}
print(run(100000));
