function subtract(a, b) { return a - b; }
var sum = 0;
for (var i = 0; i < 1000000; i++) sum += subtract(3, 2);
print(sum);
