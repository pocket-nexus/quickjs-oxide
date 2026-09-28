function field(value) { return value.length; }
var sum = 0;
for (var i = 0; i < 1000000; i++) sum += field("abc");
print(sum);
