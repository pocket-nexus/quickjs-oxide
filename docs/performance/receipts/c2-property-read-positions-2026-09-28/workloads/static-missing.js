function field(object) { return object.missing; }
var object = { x: 7 };
var count = 0;
for (var i = 0; i < 1000000; i++) if (field(object) === undefined) count++;
print(count);
