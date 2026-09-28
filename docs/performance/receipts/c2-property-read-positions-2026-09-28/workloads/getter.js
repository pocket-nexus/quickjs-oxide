var calls = 0;
var object = { get x() { calls++; return 3; } };
var sum = 0;
for (var i = 0; i < 10000; i++) sum += object.x;
print(sum + calls);
