print((function(n, operand) {
  var sum = 0;
  for (var i = 0; i < n; i++) sum += operand - 2;
  return sum;
})(1000000, true));
