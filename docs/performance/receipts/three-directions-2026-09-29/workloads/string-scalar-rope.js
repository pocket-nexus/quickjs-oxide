function work(n) {
    var leaf = 'x'.repeat(1024), sum = 0;
    for (var i = 0; i < n; i++) {
        var chunk = leaf + leaf;
        sum += 'a'.concat(chunk).length;
    }
    return sum;
}
var result = work(40000);
if (result !== 81960000) throw new Error('wrong rope concat checksum: ' + result);
print(result);
