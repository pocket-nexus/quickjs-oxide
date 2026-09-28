function field(object) { return object.x; }
var object = { x: 7 };
var sum = 0;
for (var i = 0; i < 1000000; i++) sum += field(object);
print(sum);
