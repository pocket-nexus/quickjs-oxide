// Hit with retained values: alternate two objects while preserving the hole.
function work(n) {
    var x = {value: 3}, y = {value: 7}, a = [x, , 2];
    for (var i = 0; i < n; i++) a[0] = i % 2 ? x : y;
    return a[0].value + ':' + a.length + ':' + Object.hasOwn(a, '1');
}
print(work(100000));
