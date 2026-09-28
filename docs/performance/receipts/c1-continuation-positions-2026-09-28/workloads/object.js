function subtract(a, b) { return a - b; }
var operand = { valueOf() { return 3; } };
var sum = 0;
for (var i = 0; i < 100000; i++) sum += subtract(operand, 2);
print(sum);
