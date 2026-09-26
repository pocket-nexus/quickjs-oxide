function work(n) {
    var a = [], sum = 0;
    for (var j = 63; j >= 0; j = j - 1) a[j] = j + 1;
    for (var i = 0; i < n; i = i + 1) sum = sum + a[i & 63];
    return sum;
}
print(work(1280000));
