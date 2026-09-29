var calls = 0;
var tail = { toString: function () { calls++; return "z"; } };
var sum = 0;
for (var i = 0; i < 20000; i++) {
  sum += "a".concat("x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", "x", tail).length;
}
print(sum + ":" + calls);
