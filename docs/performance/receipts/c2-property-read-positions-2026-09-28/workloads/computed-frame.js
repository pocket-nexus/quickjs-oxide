var object = { x: 7 };
var key = "x";
var sum = 0;
for (var i = 0; i < 1000000; i++) sum += object[key];
print(sum);
