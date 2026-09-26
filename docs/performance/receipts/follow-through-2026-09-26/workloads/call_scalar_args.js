function add4(a, b, c, d) {
    return a + b + c + d;
}
function work(n) {
    var sum = 0;
    for (var i = 0; i < n; i = i + 1) sum = sum + add4(i & 3, 2, 3, 4);
    return sum;
}
print(work(800000));
