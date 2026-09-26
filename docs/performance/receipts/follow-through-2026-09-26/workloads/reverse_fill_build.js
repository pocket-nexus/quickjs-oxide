function work(n) {
    var sum = 0;
    for (var k = 0; k < n; k = k + 1) {
        var a = [];
        for (var j = 63; j >= 0; j = j - 1) a[j] = j + 1;
        sum = sum + a[0] + a[31] + a[63] + a.length;
    }
    return sum;
}
print(work(3000));
