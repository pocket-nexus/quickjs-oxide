function compare(o, y) { if (o.x < y) return 1; return 2; }
var operand = { x: "1" };
var sum = 0;
for (var i = 0; i < 100000; i++) sum += compare(operand, 2);
print(sum);
