function work(n) {
    var sum = 0;
    for (var i = 0; i < n; i++) sum += 'a'.concat('b', i & 7).length;
    return sum;
}
var result = work(150000);
if (result !== 450000) throw new Error('wrong scalar concat checksum: ' + result);
print(result);
