// Hit: an intentional hole keeps the Array materialized throughout this loop.
// A read-only length still permits overwriting existing writable indices.
function work(n) {
    var a = [0, , 2];
    Object.defineProperty(a, 'length', {writable: false});
    for (var i = 0; i < n; i++) a[0] = i;
    return a[0] + ':' + a.length + ':' + Object.hasOwn(a, '1');
}
print(work(200000));
