function element(object, key) { return object[key]; }
var object = { x: 7 };
var sum = 0;
for (var i = 0; i < 1000000; i++) sum += element(object, "x");
print(sum);
